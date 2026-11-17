# 前后端兼容技术债审计

> 审计日期：2026-11-04
> 审计范围：`src-tauri/src/protocol/dto/**`、`src-tauri/crates/assistant-config/**`、`src-tauri/src/cmd/**`、`src/**` 中消费这些契约的 service / store / 类型文件
> 审计方法：`#[serde]` / `#[ts]` 派生全量检索 + `#[command]` 签名核对 + 生成契约与手写类型逐字段对照 + 前端消费点与导出别名反查 + 失败路径推演
> 证据基准：只采当前工作区源码。dto-frontend-backend-sync-audit（已归档）只作为历史清单追溯，不作为现状依据
> 引用口径：行号以本次审计时的源码为准；行号会随迭代漂移，定位优先用「文件名::类型名::字段名」锚点
> 修订记录：
> - 2026-11-04 复核修正 D8 证据与 C4 测试方案，补充 C7 死代码清单与复核统计口径；
> - 2026-11-05 复核补齐漏项：vault 错误码与 C6 统计口径修正、C3 补「前端吞异常」、C4 补残余风险与排除清单防僵尸、新增 C8（快捷键默认值三份清单）与 C9（`theme` 配置值域无单源）、§5 补护栏自身约束（禁用快照 / 无 CI）
> - 2026-11-06 关闭 C5：7 个服务层手写副本删除并改接生成契约，同时修掉未登记的 `get_mouse_position` `Option` 空值漂移
> - 2026-11-06 关闭 `compact_threshold_percent` 迁移窗口：删除 `AppConf::legacy_compact_threshold_percent` 与 `migrate()` 内的搬运逻辑，`migrate()` 保留空实现 + 迁移模板注释；原「明确不列为死代码」条目改为已移除，C3/C4 中对该迁移字段的引用同步改写
> - 2026-11-12 关闭 C1 / C2 / C3 / C4 / C7 / C8 / C9：`theme` 与 `resolved` 收窄为封闭枚举并在后端解析、错误码映射改编译期穷举、`patch_conf` 命名转换补用例、配置 DTO 字段集合测试覆盖四个结构体、死代码与 `Omit` 垫片删除、快捷键默认值改生成单源；同时修掉未登记的 `AppConf::workspace_root` 死字段。数字类结论（错误码条数、`Result<_, String>` 处数）不再逐条写死，改为「以生成物/检索命令为准」，避免下一轮复核时先怀疑文档本身
> - 2026-11-12 口径修正：新增「修法必须落到既有设施」要求——补日志写 `createLogger("<scope>")`（`src/services/logger.ts`，53 个文件在用），补测试写进 `cargo test` / `vitest` 且注明可执行范围；不写「补一条日志/补一个测试」这类无载体表述
> - 2026-11-17 C6 立项：实施规格拆到 [命令错误契约规格](../../store/command-error-contract-spec.md)（通用 `CommandErrorDto`、错误码单源、首批 chat / agent 六命令），本文 C6 节与 §5 阶段 8 只保留现状分析与指向，不再复述修法细节。**立项不改变 C6 的开放状态**：首批只覆盖 chat / agent，conversation / provider / storage / config 等域仍是字符串错误，关闭判据（`Result<[^>]*, String>` 匹配数降为 0）写在规格 §6.1

---

## 0. 结论摘要

1. 上一轮（2026-10-30 / 11-03）审计登记的 D1、D2、D4、D8、D9、D11 **六项已确认关闭**，见 §2。
2. C 类 9 项中 **C1 / C2 / C3 / C4 / C5 / C7 / C8 / C9 已全部关闭**（C5 于 2026-11-06，其余于 2026-11-12），见各节首行状态标记与 §2.1。
3. 仍开放 **1 项**：C6（chat 域命令错误仍是 `Result<_, String>`），属结构性改造，已于 2026-11-17 立项，实施规格见 [命令错误契约规格](../../store/command-error-contract-spec.md)。立项后仍为开放：首批只迁 chat / agent，关闭判据见规格 §6.1。
4. 有 **2 项**属**无需改代码的有意例外**（role/title_source 开放集合、`tool_use_id` snake_case）；`ApiFormat` 类型保留，其 `as ApiFormat` 静默错标已于 2026-11-12 补运行时校验与回落日志（`createLogger("providerLookup")`），见 §4。
5. 判定兼容债的口径是「不同步时不产生编译错误或测试失败」。凡是靠人工记忆维持的一致性，一律记为债——不是因为当前错了，而是因为下一次改错不会被发现。
6. **数字类结论以生成物与检索命令为准**：错误码条数、`Result<_, String>` 处数这类计数会随迭代漂移，本文不再把它们写成判定依据，只保留「用什么命令复核」。

---

## 1. 判定口径

一条前后端契约满足以下任一条件即记为兼容债：

| 维度 | 债的定义 |
|---|---|
| 形状 | 同一 DTO 两端各写一份副本，或生成契约被手写副本覆盖 |
| 值域 | 一端声明宽类型（`String` / `number`），另一端按窄集合消费，且无值域守卫 |
| 缺省语义 | `Option` 在两端对「缺失」与「null」的解释不一致 |
| 错误结构 | 同类失败在不同通道返回不同结构，前端需分支处理 |
| 变更传播 | 新增/重命名字段、枚举值、命令时，漏改的一端不会被编译或测试拦住 |

复核统计口径：集合类结论按元素个数计，**但不写死条数**——错误码条数以生成物 `src/types/protocol.contract.ts::ProviderErrorCodeDto` 与 `src/vault/core/contract/vault.contract.ts::VaultErrorCode` 为准；C6 的处数按 `Result<[^>]*, String>` 的**匹配数**计（同一行多次匹配计多次），覆盖 `src-tauri/src/cmd` 下全部文件，按行统计会得到不同数字，复核时不要用 `grep -c` 直接比对。

> 2026-11-12 口径修正：原文写死的「13/13、25/25、72 处/17 文件」在复核时已全部过期（vault 生成契约已新增 DEK 版本 / 仓库禁用文件等 4 个错误码而手写侧未跟上；C6 处数继续增长）。这些数字只说明「存在不一致或持续增长」，不能作为判定依据，故从判据中移除，只保留复核方法。

---

## 2. 已关闭项（复核确认）

| 编号 | 原结论 | 现状证据 | 关闭方式 |
|---|---|---|---|
| D1 tool call 字段 | 线格式存在 `tool_calls_json`，前端不消费 | `MessageDto` 无该字段；工具调用改由 `ContentBlockDto` 的 `ToolUse` / `ToolResult` 变体承载（`src-tauri/src/protocol/dto/chat.rs::ContentBlockDto`） | I30 阶段一删除线格式 |
| D2 `Option` 缺省语义 | chat 键缺失、vault 值为 `null`，前端按 `\| null` 声明 → 运行时可能是 `undefined` | `src-tauri/src/protocol/dto/**` 下 `skip_serializing_if` 检索为 0 处；`MessageDto` / `ConversationDto` / `ChatDeltaPayloadDto` 的 `Option` 字段恒输出 `null`（该结论仅限 DTO 层；`assistant-config` 侧唯一的 `skip_serializing_if` 是迁移专用字段，2026-11-06 已随迁移窗口关闭删除，见 C3/C4） | 输出侧统一「键恒在、值 `null`」 |
| D4 输入 DTO 缺 `deny_unknown_fields` | `SendChatMessageArgsDto` 是全目录唯一例外 | `SendChatMessageArgsDto` 已带 `deny_unknown_fields`（`chat.rs`） | 补齐派生 |
| D8 vault barrel 手工漂移 | `vault.types.ts` 手工列 38 个，生成物 39 个，漏 `StrengthWarning` | `src/vault/core/vault.types.ts` 已改为 `export * from "./contract/vault.contract"` + 别名层，`StrengthWarning` 等新增类型自动带出；`src/vault/core/vaultError.ts` 等消费点仍从该 barrel 导入 | 手写清单改为 `export *`；别名改名会编译期报错，不构成静默漂移。剩余空转别名与死代码见 C7 |
| D9 `_seq` 防乱序守卫失效 | 后端序列化为 `seq`，前端读 `_seq`，守卫从未生效 | `ChatMessagesChangedPayloadDto::seq`（`src-tauri/src/protocol/dto/branch.rs`）与 `src/services/chatLifecycleEvents.ts` 的 `lastProcessedSeq` 比较键名一致 | 两端改用 `seq` |
| D11 wire 与 view model 混用 | `Message.error` 等前端本地字段混进 DTO | `src/chat/core/chatTypes.ts` 用交叉类型 `MessageDto & { error?: ChatError \| null }`，wire 不含本地状态 | 交叉类型承载本地扩展 |

### 2.1 2026-11-12 关闭的 C 类项

| 编号 | 关闭方式（均已落盘并跑过验证） |
|---|---|
| C1 | 后端新增 `resolve_theme()`，`("theme", ..)` 副作用广播前把 `system` 解析成 `light` / `dark`；`ThemeChangedPayloadDto::resolved` 改为封闭枚举 `ResolvedThemeDto`；前端删除 `src/types/events.ts` 的 `Omit` 硬窄化，直接消费生成联合类型 |
| C2 | `src/chat/core/chatErrors.ts` 的 `CODE_TO_CATEGORY` 与 `src/vault/core/vaultError.ts` 的 `VAULT_ERROR_CODE_FLAGS` 均改为 `satisfies Record<...>` 的穷举表，漏项即 `npm run typecheck` 失败；改造时手写侧漏掉的 4 个新码（`vaultDekVersionAhead` / `vaultDekVersionBehind` / `repositoryContainsForbiddenFile` / `historyDekUnavailable`）随之显形并补齐 |
| C3 | 现象一由 `amend_json_config` 的 `serde_ignored` 严格校验关闭（2026-11-06，`amend_unknown_key_rejected` 已固化）；现象三由 `dispatchConfPatch` 的 `catch` 改为 `throw` 关闭；现象二补 `src/services/__tests__/settingService.test.ts`（多词键、已是 snake_case 键、仅转顶层三层断言）。修法 4（`patch_conf` 入参改 camelCase DTO）仍为可选未做，故 `patchToSnakeCase` 暂不删除 |
| C4 | `src-tauri/tests/protocol_dto_roundtrip.rs::config_dtos_cover_every_config_field` 用泛型 helper 对 `AppConf` / `OrbConf` / `MascotConf` / `VaultConf` 四个结构体逐一比对 snake→camel 归一化后的键集合；排除清单当前为空。错映射残余风险仍按原结论显式保留 |
| C7 | `vault.types.ts` 的 4 个零引用别名已删；`ThemeChangedPayload` 的 `Omit` 垫片随 C1 删除。`patchToSnakeCase` 因 C3 修法 4 未做而保留 |
| C8 | `assistant-config::app_conf::DEFAULT_SHORTCUT_PATTERNS` 成为唯一源；`protocol_bindings.rs` 生成 `src/types/shortcutDefaults.generated.ts`，`shortcutTypes.ts` 从生成物读取默认绑定，`npm run contract:check` 的 diff 范围已纳入该文件（前置 `git ls-files --error-unmatch` 守卫，避免文件未被跟踪时 `git diff` 静默放行） |
| C9 | `AppConf::theme` 由 `String` 收窄为封闭枚举 `ThemeMode`，`AppConfDto::theme` 改为 `ThemeModeDto`，生成契约即 `system` \| `light` \| `dark` 联合；前端 `themeStore` / `settingsDispatcher` / `commandAggregation` / `initSettingSystem` 的手写三值联合与 `as` 强转全部改为消费生成类型 |
| 附 | 未登记死字段 `AppConf::workspace_root` 删除（唯一来源为 `conversations.workspace_root`，`tool_setup.rs` 明确「不读 AppConf」），同步从 `validate` 与测试移除 |

---

## 3. 开放项

优先级口径：**P1** 用户可直接触发的行为错误；**P2** 高概率静默失败或低成本的防回归改动；**P3** 结构性/维护成本，需单独立项或择机处理。

> 状态：C1 / C2 / C3 / C4 / C5 / C7 / C8 / C9 **已关闭**（C5 于 2026-11-06，其余见 §2.1）。以下各节保留原始分析与修法，作为历史记录；新读者只看首行状态标记与 §2.1 即可。当前唯一开放项为 C6。

### C1 主题事件 `resolved` 值域穿透（优先级 P1，可达缺陷）—— **已关闭 2026-11-12**，见 §2.1

**现象**：后端广播 `theme-changed` 时直接透传配置值，而配置值域是 `system | light | dark`；DTO 声明 `String`，前端却按 `light | dark` 消费。

| 环节 | 位置 | 行为 |
|---|---|---|
| 值域来源 | `assistant-config::app_conf::AppConf::validate` | `theme` 只允许 `system/light/dark` |
| 后端广播 | `src-tauri/src/cmd/config.rs` 的 `APP_SIDE_EFFECTS` 中 `("theme", ..)` | `let resolved = conf.theme.clone();` 原样发出，`"system"` 也会发出 |
| DTO | `src-tauri/src/protocol/dto/settings.rs::ThemeChangedPayloadDto::resolved` | 类型为 `String`，生成契约即 `string` |
| 前端类型 | `src/types/events.ts::ThemeChangedPayload` | `Omit<ThemeChangedPayloadDto, "resolved"> & { resolved: "light" \| "dark" }`，用 `Omit` 硬窄化 |
| 前端消费 | `src/stores/themeStore.ts::init` 的 `listen(UiEvent.ThemeChanged)` | `set({ resolved: payload.resolved })` 不做解析 |
| 渲染 | `src/main.tsx` 的 `document.documentElement.setAttribute("data-theme", resolved)` | 直接写进 DOM 属性 |

**可达路径**：`src/components/ThemeToggle.tsx` 的按钮调用 `cycleMode` → `themeStore.setMode`，而 `setMode` 同时发起两件事且都不 `await`：

```
dispatchConfPatch("app", { theme: mode })   // 落盘 → 后端 side effect 广播 theme-changed("system")
dispatchThemeChanged(mode)                  // 本地解析出 light/dark → 广播 setting:theme-changed
```

后端广播携带 `"system"`，前端监听无条件覆盖 `resolved`。若后端广播晚于本地 `dispatchThemeChanged` 到达，`data-theme` 最终为 `"system"`，Ant Design 主题取 `lightTheme` 分支（`resolved === "dark"` 才取暗色），**用户切到「跟随系统」且系统为暗色时界面停在浅色**。

**为什么设置面板路径没暴露**：设置面板走 `SettingsGroup` → `onPatch`（`await` 落盘）→ `runSideEffects("theme_change")`，顺序固定为「后端广播先到、本地纠正后到」，结果被本地覆盖修正。缺陷只在 `setMode` 这条并发路径上出现，属于「顺序碰巧正确」而非契约正确。

**影响面**：主窗口与 orb 窗口的 `resolved` 都可能被污染；`themeStore` 的类型声明让下游组件相信它只会是 `light | dark`，`data-theme` 与 CSS 变量、`antdTheme` 选择都基于该假设。

**修法（推荐 A）**：

- A. 后端广播前解析：`("theme", ..)` 中不再直接透传 `conf.theme`，而是按 `system` 取当前窗口系统主题解析成 `light` / `dark`；同时把 `ThemeChangedPayloadDto::resolved` 改为封闭枚举（如 `ResolvedThemeDto`），生成契约即为联合类型。前端 `src/types/events.ts::ThemeChangedPayload` 的 `Omit` 硬窄化随之删除（见 C7）。代价是 Rust 侧需要一次系统主题读取，且该 DTO 的 round-trip 测试要跟着改。实现要点：side effect 闭包已持有 `app: &tauri::AppHandle`，可用 `app.get_webview_window(WINDOW_MAIN)` 后调 `window.theme()`（Tauri 2，返回 `Option<Theme>`），无需另造系统主题 API；新增 `ResolvedThemeDto` 后**必须**在 `src-tauri/tests/protocol_bindings.rs` 的 `declarations` 数组与 `use` 导入里各加一行，否则该生成器测试会因「漏注册」直接失败。
- B. 前端兜底：`listen` 回调里对 `"system"` 走 `resolveTheme("system", systemTheme)` 再 `set`，并把 `ThemeChangedPayload` 的类型窄化删掉。改动小，但「后端发非法值」这件事被固化下来，后续任何新消费方都要重复兜底。

**与 C9 的关系**：本项只覆盖事件通道的 `resolved`；配置通道的 `theme` 值域仍是 `String` 且在前端有十余处手写三值枚举，见 C9。两者共用同一个封闭枚举与同一次 `npm run contract:gen`，建议同批次完成。

**验证**：

- 单测：若采用修法 B，构造 `resolved: "system"` 的载荷（需绕过类型，用 `as` 或直接构造原始对象）投递给 `listen(UiEvent.ThemeChanged)` 回调，断言 `resolved` 落在 `light | dark`。`themeStore.init` 里的 `win.onThemeChanged` 已按系统主题做 `resolveTheme`，本次只需修 `listen` 这条覆盖路径；若采用修法 A，则改为断言 `listen` 直接采用后端已解析的值，不再需要前端兜底分支；
- Rust 测试：`ThemeChangedPayloadDto` round-trip 增补 `"light"` 用例（现有测试只覆盖 `"dark"`，见 `src-tauri/tests/protocol_dto_roundtrip.rs::setting_dtos_roundtrip`）；若采用修法 A，`resolved: "system"` 应断言为**反序列化失败**（值域已收窄），而不是继续 round-trip。该测试位于集成测试目标，跑 `cargo test -p assistant-app --test protocol_dto_roundtrip`；`--test protocol_bindings` 是契约生成器，不会执行它；
- 契约：`npm run contract:gen` 后确认 `src/types/protocol.contract.ts` 中 `ThemeChangedPayloadDto.resolved` 已从 `string` 变为联合类型，且 `src/types/events.ts` 的 `Omit` 窄化已删；
- 手测：`ThemeToggle` 在系统暗色下切到「跟随系统」，确认 `data-theme="dark"` 且界面不变浅。

---

### C2 错误码镜像缺编译期护栏（优先级 P2）—— **已关闭 2026-11-12**，见 §2.1

**现象**：两个域的前端错误码列表与生成契约人工维持一致，漏项不会报错。

| 域 | 手写列表 | 生成契约 |
|---|---|---|
| chat | `src/chat/core/chatErrors.ts::CODE_TO_CATEGORY`（`satisfies Record<ChatErrorCode, ...>`） | `src/types/protocol.contract.ts::ProviderErrorCodeDto`（条数以生成物为准） |
| vault | `src/vault/core/vaultError.ts::VAULT_ERROR_CODE_FLAGS`（`satisfies Record<VaultErrorCode, true>`） | `src/vault/core/contract/vault.contract.ts::VaultErrorCode`（条数以生成物为准） |

**失效表现**：

- chat：后端新增错误码后，`isChatError` 返回 `false`，`toChatError` 把真实错误归一成 `InternalError`，`code` / `retryable` / `retryAfterMs` 全部丢失，UI 展示「未知错误」且不再提供重试倒计时；
- vault：未知码在 `vaultErrorMessage` 走 `String(value)`，用户看到 `[object Object]`。

两端当前**值完全一致**，所以这不是现网缺陷，而是「下一次改错不会被发现」。

**修法**：把数组换成穷举映射表，让 TypeScript 自己校验完整性。

- chat：`const CHAT_ERROR_CODE_MAP: Record<ProviderErrorCodeDto, ChatErrorCode>`，再反向派生集合；同时用 `Exclude<ChatErrorCode, ProviderErrorCodeDto>` 断言多余项为空；
- vault：`Record<VaultErrorCode, true>` 形式的查表对象替代 `readonly VaultErrorCode[]`，`includes` 改为 `code in TABLE`；
- chat 还有一处同类映射：`src/chat/core/chatErrors.ts::inferCategory` 的 `codeToCategory` 当前是 `Partial<Record<ChatErrorCode, ChatErrorCategory>>`，虽已列全 13 项但类型不强制，新增错误码会静默落到 `Unknown`；改成 `Record<ChatErrorCode, ChatErrorCategory>`。

三者都是纯类型层改动，运行时行为不变，且漏项时 `npm run typecheck` 直接失败。

**写 `Record` 时的两个类型细节**：

- `ChatErrorCode` 是 TS `enum`（值为字符串字面量），`ProviderErrorCodeDto` 是字符串联合。`Record<ProviderErrorCodeDto, ChatErrorCode>` 的 key 用字面量、value 用 `ChatErrorCode.Xxx`；不要反过来用 `Record<ChatErrorCode, ProviderErrorCodeDto>`，因为 `enum` 作 key 时会被当作数值索引处理，漏项检测失效；
- 若保留 `enum`，`isChatError` 不能再靠 `Object.values(ChatErrorCode).includes(x as ChatErrorCode)` 之类的断言通过，改成查表后自然消除这处断言。

**验证**：`npm run typecheck`；在映射表里临时删一项确认编译报错，再恢复。

---

### C3 `patch_conf` 未知键静默丢弃，且读写命名方向相反（优先级 P2）—— **已关闭 2026-11-12**（修法 4 未做，见 §2.1）

**现象一：未知键无提示**。四个配置结构体（`AppConf` / `OrbConf` / `MascotConf` / `VaultConf`）的 `amend()` 都是「序列化自身 → 合并 patch → `from_str` 重新反序列化」，而配置结构体**故意不加** `deny_unknown_fields`（配置文件需要向前兼容，老版本读到新配置不能直接失败）。代价是 patch 里拼错的键被合并进去、反序列化时被忽略、保存时消失，**命令返回 `Ok`**，前端 `dispatchConfPatch` 认为保存成功，UI 显示「已保存」但值没变。

不能简单给配置结构体加 `deny_unknown_fields`——那会让旧版本读到新版本写出的配置直接加载失败，破坏向前兼容。正确做法是在 `amend()` 里做「已知键集合」校验：白名单从静态字段清单（或 `Default` 实例的序列化结果）取，并显式补上迁移别名；patch 顶层出现集合外的键就返回 `Err`，未知键在 `from_str` 之前就被拦住。不要直接用当前实例的 `serde_json::to_value(&self)`，原因见下方修法第 1 条。

**现象二：同一配置组读写命名方向相反**。

| 方向 | 通道 | 命名 |
|---|---|---|
| 读 | `get_app_conf` / `get_orb_conf` / `get_mascot_conf` / `get_vault_conf` → `*ConfDto` | `rename_all = "camelCase"`，线格式 camelCase |
| 写 | `patch_conf` → `serde_json::Value` → `amend()` | 配置结构体无 `rename_all`，线格式 snake_case；前端靠 `src/services/settingService.ts::patchToSnakeCase` 逐键转 |

`patchToSnakeCase` 只转顶层键，且对已经是 snake_case 的键原样保留——这条「刻意」既没有注释也没有测试。它与现象一叠加后形成同一种失败模式：命名转换一旦和后端不一致，保存会静默变成空操作。

**现象三：前端吞掉 `Err`，失败时把乐观值留在界面上**。`src/settings/core/settingsDispatcher.ts::dispatchConfPatch` 的 `catch` 只 `log.error` 不 rethrow；而 `src/settings/shared/sections/SettingsGroup.tsx` 的 `onPatch` 一旦 `await` 返回就弹「已保存」。两种失败因此都表现成「界面显示已保存」：

| 失败类型 | 后端返回 | 前端表现 |
|---|---|---|
| 未知键 / 命名不匹配（现象一） | `Ok`（被忽略） | `loadConf` 回读 → 值弹回旧值，但「已保存」已弹出 |
| 类型不匹配 / `validate()` 失败 | `Err` | 异常被吞 → `loadConf` 不执行 → 乐观值留在 store，界面是新值、落盘是旧值，重启后丢失 |

第二行比现象一更隐蔽：它让**任何** `patch_conf` 失败都退化成「界面已保存、重启丢失」，且除日志外没有用户提示。修现象一只能消掉第一行，第二行需要单独处理。

**修法**：

1. `assistant-config` 四个 `amend()` 统一加顶层键白名单校验，未知键 `Err`（错误信息里带键名）。白名单从 `Default`/静态字段清单派生，不要用当前实例的 `serde_json::to_value(&self)`：实例序列化结果会随字段的 `skip_serializing_if` 与 `Option` 状态变化（`AppConf` 的迁移字段已于 2026-11-06 删除，但今后新增迁移字段仍会重现这个问题）；迁移窗口内需显式允许旧键别名；
2. 改成 `Err` 会与既有测试冲突：`assistant-config/tests/app_conf.rs::amend_unknown_key_ignored` 与 `assistant-config/tests/vault_conf.rs::amend_unknown_key_ignored` 当前把「忽略未知键」固化为预期行为，必须同步改为断言报错；
3. `patchToSnakeCase` 补注释说明「仅顶层、`keybindings` 内层不转」的理由，并补一个最小单测（含一个多词字段与一个已是 snake_case 的键）；
4. 可选（成本更高）：`patch_conf` 入参改为按 group 反序列化成对应 `*ConfDto`（camelCase + `deny_unknown_fields`），后端再转内部结构体，读写命名就此统一。此项会动到 `patch_conf` 的签名与四个 `apply_*_conf`，建议单独立项；完成后 `patchToSnakeCase` 成为死代码，应删除（见 C7）。
5. 修现象三：`dispatchConfPatch` 的 `catch` 至少 `throw`（或返回失败结果），让 `SettingsGroup` 的失败分支生效并回滚乐观值；若要给用户提示，在 `SettingsGroup` 的 `catch` 里补一条错误提示文案。注意 `dispatchConfPatch` 的调用点不止设置页（`ThemeToggle`、`dispatchKeybindingsReset` 也走它），改 `throw` 前要逐个确认调用方是否已处理异常，避免把静默失败变成未捕获 rejection。

**验证**：`cargo test -p assistant-config`（新增 amend 未知键用例）；`npm run test`（`settingService` 命名转换用例 + `dispatchConfPatch` 失败路径用例：mock `patchConf` 抛错，断言不弹「已保存」且 store 回滚到落盘值）。

---

### C4 配置 DTO 与配置结构体的字段漂移无测试（优先级 P2）—— **已关闭 2026-11-12**，见 §2.1

**现象**：`AppConfDto` / `OrbConfDto` / `MascotConfDto` / `VaultConfDto` 与 `assistant-config` 中对应结构体之间是**手写 `From` 逐字段映射**，没有编译期约束，也没有任何测试。`src-tauri/src/protocol/dto/config.rs` 下 `#[test]` 检索为 0 处。

给 `AppConf` 加一个字段、忘记加进 `AppConfDto` 的后果：配置能落盘、能读到，但前端永远看不到该字段，设置页对应项渲染为空，且没有任何报错。

**注意：本项的副本只在 Rust 侧，前端已经没有了**。`src/settings/core/types/confTypes.ts` 是 `export type AppConf = AppConfDto` 之类的纯别名（11 行，全部指向生成契约），不存在第二份字段清单。所以 C4 不能靠「删前端副本」解决，只能在 Rust 侧加测试——这也是它被单列的原因。

**为什么不能直接写「字段集合相等」测试**：有两个约束必须先处理。

- 配置结构体没有 `rename_all`，序列化键是 snake_case；`*ConfDto` 是 camelCase。直接把 `serde_json::to_value(&conf)` 与 DTO 序列化结果的键集合比较，所有多词字段都会不等，必须先做 snake→camel 归一化；
- 迁移专用字段会让键集合随实例状态变化（`skip_serializing_if` + 旧键 `rename`）。`AppConf::legacy_compact_threshold_percent` 已于 2026-11-06 随迁移窗口关闭删除，当前四个结构体都没有这类字段，排除清单应为空；但迁移机制本身保留（`migrate()` 留有模板注释），今后新增迁移字段时仍必须把旧键静态写进排除清单，不能用实例序列化结果推导。

**修法**：

- 在 `src-tauri/tests/protocol_dto_roundtrip.rs` 中，用各结构体的构造入口（如 `AppConf::new()`）或显式全字段构造，序列化后做 snake→camel 归一化，再与 `*ConfDto::from(conf)` 序列化结果的键集合比对；排除清单当前为空，若今后出现迁移字段只允许写静态条目并注释迁移用途。**不要放 lib 的 `#[cfg(test)]`**：Windows 上 `assistant-app` lib 测试二进制缺 Common Controls v6 清单，启动即 `STATUS_ENTRYPOINT_NOT_FOUND`，lib 单测在本机不可执行；
- **排除清单必须双向断言**：除「被排除键确实出现在源侧序列化结果」外，还要断言排除清单里的每个键仍存在于结构体字段清单，否则迁移字段删除后排除清单会变成只增不减的死名单——那本身就是新债。断言失败信息里要求「键已不存在，请删除该条目」；
- DTO 的 camelCase 输出断言放 `src-tauri/tests/protocol_contract.rs`（已有 `conversation_dto_outputs_camel_case` 等先例）。`src-tauri/tests/protocol_bindings.rs` 是契约生成器，会重写 `src/types/protocol.contract.ts`，不要往里加 serde 断言。

**残余风险（必须写明，避免「已关闭」被误读为「已完备」）**：键集合比对只能抓「单边新增/删除字段」，**抓不到错映射**。例如 `AppConfDto::from` 里把 `language` 写成 `c.theme`（两侧都是 `String`），编译通过、键集合相等、测试全绿。要覆盖错映射只有两条路：给全部字段填唯一哨兵值做字段级 fixture（四个结构体合计数十个字段，是真实的新副本，维护成本高），或用 derive/生成让 `From` 不再是手写的（需要 proc-macro 基础设施，当前工作区没有 `syn` / `quote` 依赖）。**本项按「接受键集合测试 + 显式记录错映射残余风险」处理**；若后续引入生成方案，应回头替换本测试而不是叠加。

**验证**：`cargo test -p assistant-app --test protocol_dto_roundtrip --test protocol_contract`；临时删 `AppConfDto` 一个字段确认测试失败，再恢复。`--test protocol_bindings` 不执行任何断言，不能作为本项的验证命令。

---

### C5 服务层手写 DTO 副本（优先级 P3）—— **已关闭 2026-11-06**

三个 service 文件各自手写了一份与生成契约同构的类型，其中 `storageService` 的四个类型与生成契约**完全同名**（未导入生成类型，因此不构成编译冲突，只是两份独立副本）：

| 文件 | 手写类型 | 生成契约中的对应类型 |
|---|---|---|
| `src/services/settingService.ts` | `InstallAssetResult` | `InstallMascotAssetResultDto` |
| `src/services/storageService.ts` | `AttachmentSummary` | `AttachmentSummary`（同名） |
| `src/services/storageService.ts` | `ConversationAttachmentSummary` | `ConversationAttachmentSummary`（同名） |
| `src/services/storageService.ts` | `StorageBreakdownDto` | `StorageBreakdownDto`（同名） |
| `src/services/storageService.ts` | `AttachmentDeleteResultDto` | `AttachmentDeleteResultDto`（同名） |
| `src/services/orbService.ts` | `MousePosition` | `MousePositionDto` |
| `src/services/orbService.ts` | `ScreenSize` | `ScreenSizeDto`（内层为 `ScreenRectDto`） |

当前结构一致，所以不是缺陷；风险在「后端加字段 → 生成契约更新 → 手写副本不变 → 前端读不到新字段，且结构相同时 TS 不会提醒」。

**修法**：消费点直接改成 `import type { ... } from "@/types/protocol.contract"`，**删除**手写的 `interface` 副本；调用点用 `import { StorageBreakdownDto as StorageBreakdown }` 或直接改名。不要保留 `export type X = XDto` 这类第二份名字——那只是把结构副本降级成名字副本，最终会变成无人引用的死代码（见 C7）。

同一套做法在配置域**已经落地**：`src/settings/core/types/confTypes.ts` 全部是 `export type AppConf = AppConfDto` 形式的别名，没有第二份字段清单，说明这个模式在本项目可用且低风险。C5 只是把它推广到 service 层。

**验证**：`npm run typecheck`；删掉生成契约里某个字段，确认所有消费点编译失败（即证明类型真的连上了）。

**2026-11-06 已关闭**：

- 三个 service 的手写 `interface` 全部删除，改为 `import type { ... } from "@/types/protocol.contract"`。四个 storage 类型与契约同名，无需改名；`InstallAssetResult` → `InstallMascotAssetResultDto`、`MousePosition` → `MousePositionDto`、`ScreenSize` → `ScreenSizeDto`，改名后无旧名消费点残留；
- `src/settings/pages/StorageSettings.tsx` 的三个类型改从契约导入（原先从 `storageService` 转出，删除副本后会一并失效）；
- **顺带发现并修掉一处真实漂移（文档原先未登记）**：`get_mouse_position` 后端签名是 `Option<MousePositionDto>`，取不到位置时返回 `null`，而手写副本声明为 `Promise<MousePosition>`。`src/orb/OrbCoord.ts::getMouseLogicalPos` 直接读 `pos.clientX`，`null` 时抛 `TypeError`，被 `src/orb/OrbPanel.tsx` 的 `try/catch` 吞掉后静默降级为「鼠标已离开 → 折叠面板」。改为 `Promise<MousePositionDto | null>` 后 `getMouseLogicalPos` 返回 `LogicalPos | null`，`OrbPanel` 显式判空（`null` 仍走折叠分支，与原 catch 语义一致，但不再依赖异常）；
- 验证：`npm run typecheck`、`npm run lint`、`npm run test`（326 项通过）。

**残余风险**：`ScreenSizeDto` 的内层 `screen` 引用 `ScreenRectDto`，手写副本里是内联对象字面量；结构相同所以未暴露，但这类「内层是否复用命名类型」的差异，键集合测试抓不到，只能靠直接引用生成契约来消除——本次改动已消除。

---

### C6 错误通道不对称：chat 域仍是 `Result<_, String>`（优先级 P3，结构性）—— **已立项 2026-11-17，仍开放**，实施规格见 [命令错误契约规格](../../store/command-error-contract-spec.md)

**现象**：`src-tauri/src/cmd` 下 17 个文件共 72 处 `Result<_, String>`（含内部 helper），其中 chat / conversation / provider / storage / config 域的命令全部是纯字符串错误；只有 vault 域（`src-tauri/src/cmd/vault/sync.rs` 等）返回结构化 `VaultErrorDto`。数量比上一版（69 处 / 16 文件）增加，说明新增命令仍在沿用旧签名。

**影响**：同一类失败在两条通道上表现不同。

| 通道 | 结构 | 前端可用的信息 |
|---|---|---|
| 流内错误 | `ProviderErrorPayloadDto` | `code` / `retryable` / `retryAfterMs` / `httpStatus` / `provider` |
| 命令级错误（chat 域） | `String` | 只有文案；`toChatError` 归一成 `InternalError`，失去重试倒计时与分类 |

即「发送消息」本身失败（如 provider 限流、鉴权失败）与「流中断」失败，在 UI 上的呈现和可操作性不一致。

**修法**：定一个通用 `CommandErrorDto`（`code` / `message` / `retryable` / `retryAfterMs` / `details`），新命令按新契约写；存量命令不必一次性改完——前端 `toChatError` 已同时接受 `string` 与结构化对象，可以逐个迁移而不破坏调用点。**建议单独立项**，因为它会触及 `#[command]` 签名、前端 chat / conversation 等域的调用点（如 `src/services/chatService.ts`）返回类型与错误分支。

> 2026-11-17 立项时对修法的两处调整（详见规格 §1.3）：`details` 去掉，改由前端 `toChatError(extra)` 本地注入；`VaultErrorDto` 不合并。首批只迁 chat / agent 六命令，**conversation / provider / storage / config 等域仍是字符串错误**，因此首批完成只算「部分关闭」——C6 完全关闭的判据是 `src-tauri/src/cmd` 下 `Result<[^>]*, String>` 匹配数降为 0，见规格 §6.1。

**验证**：迁移单个命令（如 `send_chat_message`）后，确认前端仍走 `toChatError` 且能拿到 `retryAfterMs`。

---

### C7 遗留死代码与一次性兼容垫片（优先级 P3）—— **已关闭 2026-11-12**，见 §2.1

**现象**：几轮契约迁移留下了不再被消费的导出、别名和临时转换层。它们不报错、不影响运行，但会让下一次审计误以为仍是有效入口，也会在「统一命名」时被误改。以下清单已逐一核对消费点。

| 位置 | 遗留物 | 判定证据 | 处理 |
|---|---|---|---|
| `src/vault/core/vault.types.ts` | `EntryPreviewField` / `PendingRemovalEntry` / `TotpInput` / `UnreadableReason` 四个别名 | barrel 之外引用数为 0；对应原类型仍由 `export *` 导出 | **已删除（2026-11-12）**；其余别名仍有消费点，暂不能整体删除 barrel |
| `src/services/settingService.ts` | `InstallAssetResult` | C5 改为生成类型后成为重复副本 | 已随 C5 删除（2026-11-06） |
| `src/services/storageService.ts` | `AttachmentSummary` / `ConversationAttachmentSummary` / `StorageBreakdownDto` / `AttachmentDeleteResultDto` 四个手写接口 | 与生成契约同名同构 | 已随 C5 删除（2026-11-06） |
| `src/services/orbService.ts` | `MousePosition` / `ScreenSize` | 对应 `MousePositionDto` / `ScreenSizeDto` | 已随 C5 删除（2026-11-06） |
| `src/types/events.ts` | `ThemeChangedPayload` 的 `Omit` 窄化 | C1 修法 A 后 `resolved` 已是封闭枚举 | **已删除（2026-11-12）** |
| `src/services/settingService.ts` | `patchToSnakeCase` | 仅 C3 修法 4（`patch_conf` 入参改为 camelCase DTO）完成后失效 | **保留**：修法 4 未做，该函数仍是唯一命名转换点，已由 `src/services/__tests__/settingService.test.ts` 固化行为；届时删除函数与单测 |

**明确不列为死代码**（避免后续审计重复挖）：

- ~~`assistant-config::app_conf::AppConf::legacy_compact_threshold_percent`：迁移专用字段~~ 已于 2026-11-06 随迁移窗口关闭删除（字段与 `migrate()` 内的搬运逻辑一并移除，`migrate()` 保留为空实现 + 迁移模板注释）；
- `assistant-storage` 的 `messages.tool_calls_json`：DB 预留列（当前写入恒 NULL，`db.rs` 有「禁止 DROP」注释），不是 wire 字段残留。

**验证**：`npm run typecheck`、`npm run lint`；删除别名后全局检索无残留引用；多删一项会直接编译失败，可作为反向验证。

---

### C8 快捷键默认值在三处清单人工维持（优先级 P2）—— **已关闭 2026-11-12**，见 §2.1

**现象**：同一组默认键位写在三处，且没有任何跨语言断言。

| # | 位置 | 内容 | 数量 |
|---|---|---|---|
| ① | `src-tauri/crates/assistant-config/src/app_conf.rs::default_keybindings` | id + 绑定串 | 3（`toggle_orb` / `toggle_main` / `new_chat`） |
| ② | `src-tauri/src/setup.rs::SHORTCUT_SPECS` | id + 动作回调 | 3（同上） |
| ③ | `src/settings/core/types/shortcutTypes.ts::SHORTCUT_DEFINITIONS` | id + 绑定串 + 文案 + `kind` | 4（3 个 `global` + `commandPalette` 为 `in-app`） |

`DEFAULT_KEYBINDINGS`（同文件，由 ③ `Object.fromEntries` 派生）不算第四份。① 的 doc 注释自称「默认快捷键的唯一来源，Rust 注册与前端默认值必须保持一致」——即注释本身承认靠人工维持；而 `assistant-config/tests/shortcut.rs` 只覆盖 ① 的内部行为，**没有任何跨语言一致性断言**。

**失效表现**：

- 只改 ① 漏改 ③：设置页显示旧键位，后端按新键位注册，用户在设置页看到的与实际生效的不一致；
- 只改 ③ 漏改 ①：`dispatchKeybindingsReset` 写入的是 ③ 的值，而 `setup::register_global_shortcuts` 用 `defaults = default_keybindings()` 作为 `resolve_bindings` 的回退基准；两侧不一致时「重置」的实际效果会随配置状态漂移；
- **三份清单已经发生过实际不对称**：③ 多出 `commandPalette`，而前端「重置快捷键」会把它写进 `AppConf.keybindings`，① 的默认值里没有这个键。当前无害（`SHORTCUT_SPECS` 只解析自己的 3 个 id），但这正说明「靠人记」已经开始失效。

**修法（按优先级）**：

- 首选：扩展 `src-tauri/tests/protocol_bindings.rs` 的生成器，让它同时发射默认键位常量（当前只发射类型声明），前端从生成物读 `DEFAULT_KEYBINDINGS`，③ 只保留文案与 `kind`。源清单从 3 份降到 2 份，绑定串不可能漂移；
- 次选：vitest 里通过一个只读命令取 Rust 侧默认键位再与 ③ 比对。不新增文件，但新增一个 wire 面，且需要 Tauri mock；
- **禁止**：把期望键位硬编码进测试，或用快照文件 + `-u` 刷新——两者都会让副本退化成 fail-silent，见 §5 的护栏约束。

若走次选，断言需按 ③ 的 `kind === "global"` 过滤，并显式固化「Rust 默认键位 == TS 的 global 子集」这条假设（当前成立）。

**验证**：改 `default_keybindings()` 中任一键位，确认测试失败；`npm run test`、`cargo test -p assistant-config --test shortcut`。

---

### C9 `theme` 配置值无值域，前端多处手写三值枚举（优先级 P2）—— **已关闭 2026-11-12**，见 §2.1

**现象**：C1 修的是 `theme-changed` 广播的 `resolved` 通道；但 `theme` 这个**配置值本身**同样是无值域的 `String`，前端却按三值联合消费。`system | light | dark` 在源码里以不同形态重复，下表列出 12 处（末行含 3 张映射表）：

| # | 位置 | 形态 |
|---|---|---|
| ① | `assistant-config::app_conf::AppConf::validate` | `let valid_themes = ["system", "light", "dark"];` |
| ② | `AppConf::new` | `theme: "system".to_string()` |
| ③ | `src-tauri/src/protocol/dto/config.rs::AppConfDto::theme` | `String`，生成契约即 `string` |
| ④ | `src/stores/themeStore.ts::ThemeMode` | `"system" \| "light" \| "dark"` |
| ⑤ | `src/stores/themeStore.ts::SYSTEM_THEME_CYCLE` | 同值域数组 |
| ⑥ | `src/stores/themeStore.ts::init` | `["system","light","dark"].includes(conf.theme)` 再兜底 `"system"` |
| ⑦ | `src/settings/core/settingsDispatcher.ts::dispatchThemeChanged` | 参数联合 + `mode !== "system"` 分支 |
| ⑧ | `src/settings/core/field/fieldDefinitions.ts::THEME_SETTING.options` | 三个 option 的 `value` |
| ⑨ | `src/settings/shared/renderers/ThemeRadioRender.tsx::THEME_OPTIONS` | 与 ⑧ **重复的第二份 options 列表** |
| ⑩ | `src/components/commandPalette/commandAggregation.tsx` | `order` 数组 + `appConf.theme as "system" \| "light" \| "dark"` 强转 |
| ⑪ | `src/settings/core/initSettingSystem.ts` | `next.theme as "system" \| "light" \| "dark"` 强转 |
| ⑫ | `src/components/ThemeToggle.tsx` | `THEME_ICONS` / `THEME_KEYS` / `NEXT_THEME` 三张映射表 |

当前靠 ① 的 `validate()` 兜底，所以配置里不会出现非法值——这也是它没有像 C1 那样暴露的原因。风险在新增第四个模式（如 `high-contrast`）时：后端只改 ① 就能通过校验并写盘，而前端 ④⑤⑥⑦⑧⑨⑩⑪⑫ 全部不认，界面静默回落到 `"system"` 分支或渲染出空选项。

**修法（与 C1 合并做）**：

- 后端把 `theme` 收窄成封闭枚举（如 `ThemeModeDto { System, Light, Dark }`），`AppConf` 内部同步改用枚举（或保留 `String` 但在 `validate` 与 DTO 边界做转换），生成契约即为联合类型；
- 前端 ④⑤⑥⑦⑪ 改为从生成契约取联合类型，删除 `as` 强转；⑧⑨ 合并为一份（`ThemeRadioRender` 直接消费 `THEME_SETTING.options`）；⑫ 的三张表改为 `Record<ThemeMode, ...>`，新增模式时编译期报错。

**与 C1 的关系**：C1 修 `resolved`（事件通道），C9 修 `theme`（配置通道）。两者共用同一枚举定义与同一次 `npm run contract:gen`，建议同批次完成；若只做 C1，配置通道的十余处手写值域仍然存在。

**验证**：`npm run contract:gen` + `npm run typecheck`；在 Rust 枚举里临时加一个变体，确认前端穷举分支编译失败（证明护栏生效），再恢复。

---

## 4. 有意例外与待办事项

| 项 | 位置 | 现状 | 处理 |
|---|---|---|---|
| `role` / `titleSource` 为开放字符串 | `MessageDto::role`、`ConversationDto::title_source` | DB 列为 TEXT，加枚举需要数据迁移；前端按 `user` / `assistant` / `compact_summary` / `placeholder` 比较，未知值走非用户分支渲染，不会崩 | 保留。在字段处补注释「存储边界例外，值域由 DB 决定，前端必须兜底」 |
| `ContentBlockDto::tool_use_id` snake_case | `src-tauri/src/protocol/dto/chat.rs::ContentBlockDto` 的 `ToolUse` / `ToolResult` | 全仓唯一 snake_case 线格式字段（`#[serde(rename = "tool_use_id")]`），内嵌工具协议原文；前端消费点为 0 | 保留。在 `#[serde(rename)]` 处补注释「协议原文，禁止改为 camelCase」，否则后续统一命名时会被误改并破坏与工具协议的对应关系 |
| `ApiFormat` 前端联合 | `src/settings/provider/types.ts::ApiFormat` 为 `"openai" \| "anthropic" \| "local"` | 后端 `ProviderInputDto::type` 是 `String`，格式注册表也是开放集合；`providerLookup.ts` 的 `getApiFormatByType` 对未知类型回落 `"openai"` | 类型保留。**回落日志已补（2026-11-12）**：`getApiFormatByType` 在 `PRESET_MAP` 未命中时走 `createLogger("providerLookup")` 的 `log.warn`，带上 `type` 与回落后的 `format`；类型仍是 `as ApiFormat` 的开放集合断言，故不构成静默失败。剩余风险：未知格式仍会被**按 OpenAI 分支渲染**（图标、是否需要 API Key、默认 Base URL），但至少可从日志定位 |

---

## 5. 处理顺序与验证清单

建议顺序：C1 + C9（同一枚举、同一次契约生成，含 C7 中对应删除）→ C2（防回归，纯类型层）→ ~~C5（去重，含 C7 中对应删除）~~（已完成）→ C3（含 C7 中对应删除）→ C8（快捷键默认值单源）→ C4（后端测试）→ C7（死代码收尾）→ C6（另立项）。

**阶段 1–7 已于 2026-11-12 全部执行完毕**（C5 于 2026-11-06），当前只剩阶段 8（C6，另立项）。下表保留各阶段的验证命令，供回归时复用。

| 阶段 | 内容 | 验证命令（在仓库根目录执行；cargo 在 `src-tauri` 目录执行） |
|---|---|---|
| 1 | C1 主题事件 `resolved` 收窄 + C9 `theme` 配置值域单源（**已完成 2026-11-12**） | `npm run contract:gen`；`cargo test -p assistant-app --test protocol_dto_roundtrip`；`npm run typecheck`；`npm run test` |
| 2 | C2 错误码映射表（**已完成 2026-11-12**） | `npm run typecheck`；`npm run test` |
| 3 | C5 服务层复用生成类型（**已完成 2026-11-06**） | `npm run typecheck`；`npm run lint`；`npm run test` |
| 4 | C3 `amend()` 未知键校验 + 吞异常修复 + 命名转换用例（**已完成 2026-11-12**） | `cargo test -p assistant-config`；`npm run test` |
| 5 | C8 快捷键默认值单源（**已完成 2026-11-12**，走生成方案） | `npm run contract:gen`；`npm run test`；`cargo test -p assistant-config --test shortcut` |
| 6 | C4 配置 DTO 字段集合测试（**已完成 2026-11-12**，覆盖四个结构体） | `cargo test -p assistant-app --test protocol_dto_roundtrip`；`cargo test -p assistant-app --test protocol_contract` |
| 7 | C7 死代码与兼容垫片清理（**已完成 2026-11-12**，`patchToSnakeCase` 按 C3 修法 4 保留） | `npm run typecheck`；`npm run lint`；全局检索无残留引用 |
| 8 | C6 命令错误契约（**已立项 2026-11-17，仍开放**；首批 chat / agent 六命令 + 三个 helper，完成后仅「部分关闭」，关闭判据见规格 §6.1） | `npm run contract:gen`；`cargo test -p assistant-app --test protocol_dto_roundtrip`；`cargo test -p assistant-app --test protocol_bindings`；`npm run contract:check`；`npm run typecheck`；`npm run lint`；`npm run test`；`rg -c --stats "Result<[^>]*, String>" src/cmd`（在 `src-tauri` 下） |

通用收尾：`npm run contract:check`（确认生成契约与源码一致、无遗漏提交）、`npm run lint`、`npm run typecheck`、`npm run test`；并逐项确认 C7 表中的遗留物已删除，或已在「明确不列为死代码」中写明保留原因。

### 5.1 护栏自身的约束（加测试前必读）

C 类修法（加测试）会引入新副本。副本算不算债，取决于「漂移时会不会失败」，因此有三条硬约束：

1. **禁止快照**：不用 `toMatchSnapshot` / `toMatchInlineSnapshot`，也不把期望值存成文件再 `-u` 刷新。快照的「更新即通过」会把 fail-loud 副本重新变回 fail-silent，等于绕一圈把债还回去。当前 `src/` 下快照用法为 0 处，应保持。
2. **排除清单必须双向断言**：任何「允许不对称」的白名单（C4 当前为空，今后出现迁移字段时会用到）都要断言其条目仍存在于源侧，否则会变成只增不减的死名单。
3. **护栏依赖执行**：仓库当前**没有 CI 配置**（无 `.github/workflows`、无 `.gitlab-ci.yml`），`npm run contract:check` 只能靠人记得跑。C 类测试写在 `cargo test` / `vitest` 里，只有真的被执行时才有效。建议把 `cargo test`、`npm run typecheck`、`npm run test`、`npm run contract:check` 固化成提交前动作；否则新增的只是「不会被执行的副本」，那才是真正的新债。

---

## 6. 定位锚点

行号会漂移，优先按以下锚点检索：

| 主题 | 检索词 |
|---|---|
| C1 | `ThemeChangedPayloadDto`、`ResolvedThemeDto`、`resolve_theme`、`THEME_CHANGED`、`data-theme` |
| C2 | `ChatErrorCode`、`CODE_TO_CATEGORY`、`ProviderErrorCodeDto`、`VAULT_ERROR_CODE_FLAGS`、`VaultErrorCode` |
| C3 | `fn amend`、`amend_json_config`、`patchToSnakeCase`、`patch_conf`、`dispatchConfPatch`、`amend_unknown_key_rejected` |
| C4 | `assert_dto_covers_config`、`config_dtos_cover_every_config_field`、`AppConfDto` |
| C5 | `InstallMascotAssetResultDto`、`StorageBreakdownDto`、`MousePositionDto` |
| C6 | `Result<(), String>`、`VaultErrorDto`、`toChatError`、`CommandErrorDto`、`CommandErrorCodeDto`、`command-error-contract-spec` |
| C7 | `EntryPreviewField`、`PendingRemovalEntry`、`TotpInput`、`UnreadableReason`、`patchToSnakeCase`、`tool_calls_json` |
| C8 | `DEFAULT_SHORTCUT_PATTERNS`、`shortcutDefaults.generated`、`SHORTCUT_SPECS`、`SHORTCUT_DEFINITIONS`、`DEFAULT_KEYBINDINGS` |
| C9 | `ThemeMode`、`ThemeModeDto`、`SYSTEM_THEME_CYCLE`、`THEME_SETTING`、`THEME_OPTIONS`、`NEXT_THEME` |
| 日志载体 | `createLogger`、`src/services/logger.ts`（补日志一律走这里，禁止裸 `console.*`） |
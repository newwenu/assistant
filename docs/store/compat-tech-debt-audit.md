# 前后端兼容技术债审计

> 审计日期：2026-11-04
> 审计范围：`src-tauri/src/protocol/dto/**`、`src-tauri/crates/assistant-config/**`、`src-tauri/src/cmd/**`、`src/**` 中消费这些契约的 service / store / 类型文件
> 审计方法：`#[serde]` / `#[ts]` 派生全量检索 + `#[command]` 签名核对 + 生成契约与手写类型逐字段对照 + 前端消费点与导出别名反查 + 失败路径推演
> 证据基准：只采当前工作区源码。`docs/achievement3/dto-frontend-backend-sync-audit.md` 只作为历史清单追溯，不作为现状依据
> 引用口径：行号以本次审计时的源码为准；行号会随迭代漂移，定位优先用「文件名::类型名::字段名」锚点
> 修订记录：2026-11-04 复核修正 D8 证据与 C4 测试方案，补充 C7 死代码清单与复核统计口径

---

## 0. 结论摘要

1. 上一轮（2026-10-30 / 11-03）审计登记的 D1、D2、D4、D8、D9、D11 **六项已确认关闭**，见 §2。
2. 仍开放 **7 项**：C1 主题事件值域穿透是无条件可达的 UI 行为缺陷；C3 在键名拼错或命名转换不一致时同样会表现为「显示已保存但值没变」的用户可见失败；C7 是死代码与一次性兼容垫片清理；其余 4 项属「不产生编译/测试失败」的静默失效或重复维护成本。
3. 有 **2 项**属**无需改代码的有意例外**（role/title_source 开放集合、`tool_use_id` snake_case）；`ApiFormat` 类型可保留，但 `as ApiFormat` 的静默错标仍需补运行时校验与日志，属待办而非纯例外，见 §4。
4. 判定兼容债的口径是「不同步时不产生编译错误或测试失败」。凡是靠人工记忆维持的一致性，一律记为债——不是因为当前错了，而是因为下一次改错不会被发现。

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

复核统计口径：集合类结论（C2 的 13/13、26/26）按元素个数计；C6 的 69 处按 `Result<[^>]*, String>` 的**匹配数**计（同一行多次匹配计多次），按行统计会得到 87 行，复核时不要用 `grep -c` 直接比对。

---

## 2. 已关闭项（复核确认）

| 编号 | 原结论 | 现状证据 | 关闭方式 |
|---|---|---|---|
| D1 tool call 字段 | 线格式存在 `tool_calls_json`，前端不消费 | `MessageDto` 无该字段；工具调用改由 `ContentBlockDto` 的 `ToolUse` / `ToolResult` 变体承载（`src-tauri/src/protocol/dto/chat.rs::ContentBlockDto`） | I30 阶段一删除线格式 |
| D2 `Option` 缺省语义 | chat 键缺失、vault 值为 `null`，前端按 `\| null` 声明 → 运行时可能是 `undefined` | `src-tauri/src/protocol/dto/**` 下 `skip_serializing_if` 检索为 0 处；`MessageDto` / `ConversationDto` / `ChatDeltaPayloadDto` 的 `Option` 字段恒输出 `null`（该结论仅限 DTO 层；`assistant-config::AppConf::legacy_compact_threshold_percent` 仍保留 `skip_serializing_if`，见 C3/C4） | 输出侧统一「键恒在、值 `null`」 |
| D4 输入 DTO 缺 `deny_unknown_fields` | `SendChatMessageArgsDto` 是全目录唯一例外 | `SendChatMessageArgsDto` 已带 `deny_unknown_fields`（`chat.rs`） | 补齐派生 |
| D8 vault barrel 手工漂移 | `vault.types.ts` 手工列 38 个，生成物 39 个，漏 `StrengthWarning` | `src/vault/core/vault.types.ts` 已改为 `export * from "./contract/vault.contract"` + 别名层，`StrengthWarning` 等新增类型自动带出；`src/vault/core/vaultError.ts` 等消费点仍从该 barrel 导入 | 手写清单改为 `export *`；别名改名会编译期报错，不构成静默漂移。剩余空转别名与死代码见 C7 |
| D9 `_seq` 防乱序守卫失效 | 后端序列化为 `seq`，前端读 `_seq`，守卫从未生效 | `ChatMessagesChangedPayloadDto::seq`（`src-tauri/src/protocol/dto/branch.rs`）与 `src/services/chatLifecycleEvents.ts` 的 `lastProcessedSeq` 比较键名一致 | 两端改用 `seq` |
| D11 wire 与 view model 混用 | `Message.error` 等前端本地字段混进 DTO | `src/chat/core/chatTypes.ts` 用交叉类型 `MessageDto & { error?: ChatError \| null }`，wire 不含本地状态 | 交叉类型承载本地扩展 |

---

## 3. 开放项

优先级口径：**P1** 用户可直接触发的行为错误；**P2** 高概率静默失败或低成本的防回归改动；**P3** 结构性/维护成本，需单独立项或择机处理。

### C1 主题事件 `resolved` 值域穿透（优先级 P1，可达缺陷）

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

- A. 后端广播前解析：`("theme", ..)` 中不再直接透传 `conf.theme`，而是按 `system` 取当前窗口系统主题解析成 `light` / `dark`；同时把 `ThemeChangedPayloadDto::resolved` 改为封闭枚举（如 `ResolvedThemeDto`），生成契约即为联合类型。前端 `src/types/events.ts::ThemeChangedPayload` 的 `Omit` 硬窄化随之删除（见 C7）。代价是 Rust 侧需要一次系统主题读取，且该 DTO 的 round-trip 测试要跟着改。
- B. 前端兜底：`listen` 回调里对 `"system"` 走 `resolveTheme("system", systemTheme)` 再 `set`，并把 `ThemeChangedPayload` 的类型窄化删掉。改动小，但「后端发非法值」这件事被固化下来，后续任何新消费方都要重复兜底。

**验证**：

- 单测：构造 `resolved: "system"` 的载荷投递给 `listen(UiEvent.ThemeChanged)` 回调，断言 `resolved` 落在 `light | dark`。`themeStore.init` 里的 `win.onThemeChanged` 已按系统主题做 `resolveTheme`，本次只需修 `listen` 这条覆盖路径；
- Rust 测试：`ThemeChangedPayloadDto` round-trip 增补 `"system"` 与非法值用例（现有测试只覆盖 `"dark"`，见 `src-tauri/tests/protocol_dto_roundtrip.rs::setting_dtos_roundtrip`）。该测试位于集成测试目标，跑 `cargo test -p assistant-app --test protocol_dto_roundtrip`；`--test protocol_bindings` 是契约生成器，不会执行它；
- 手测：`ThemeToggle` 在系统暗色下切到「跟随系统」，确认 `data-theme="dark"` 且界面不变浅。

---

### C2 错误码镜像缺编译期护栏（优先级 P2）

**现象**：两个域的前端错误码列表与生成契约人工维持一致，漏项不会报错。

| 域 | 手写列表 | 生成契约 |
|---|---|---|
| chat | `src/chat/core/chatErrors.ts::ChatErrorCode`（13 项） | `src/types/protocol.contract.ts::ProviderErrorCodeDto`（13 项） |
| vault | `src/vault/core/vaultError.ts::VAULT_ERROR_CODES`（26 项） | `src/vault/core/contract/vault.contract.ts::VaultErrorCode`（26 项） |

**失效表现**：

- chat：后端新增错误码后，`isChatError` 返回 `false`，`toChatError` 把真实错误归一成 `InternalError`，`code` / `retryable` / `retryAfterMs` 全部丢失，UI 展示「未知错误」且不再提供重试倒计时；
- vault：未知码在 `vaultErrorMessage` 走 `String(value)`，用户看到 `[object Object]`。

两端当前**值完全一致**，所以这不是现网缺陷，而是「下一次改错不会被发现」。

**修法**：把数组换成穷举映射表，让 TypeScript 自己校验完整性。

- chat：`const CHAT_ERROR_CODE_MAP: Record<ProviderErrorCodeDto, ChatErrorCode>`，再反向派生集合；同时用 `Exclude<ChatErrorCode, ProviderErrorCodeDto>` 断言多余项为空；
- vault：`Record<VaultErrorCode, true>` 形式的查表对象替代 `readonly VaultErrorCode[]`，`includes` 改为 `code in TABLE`；
- chat 还有一处同类映射：`src/chat/core/chatErrors.ts::inferCategory` 的 `codeToCategory` 当前是 `Partial<Record<ChatErrorCode, ChatErrorCategory>>`，虽已列全 13 项但类型不强制，新增错误码会静默落到 `Unknown`；改成 `Record<ChatErrorCode, ChatErrorCategory>`。

三者都是纯类型层改动，运行时行为不变，且漏项时 `npm run typecheck` 直接失败。

**验证**：`npm run typecheck`；在映射表里临时删一项确认编译报错，再恢复。

---

### C3 `patch_conf` 未知键静默丢弃，且读写命名方向相反（优先级 P2）

**现象一：未知键无提示**。四个配置结构体（`AppConf` / `OrbConf` / `MascotConf` / `VaultConf`）的 `amend()` 都是「序列化自身 → 合并 patch → `from_str` 重新反序列化」，而配置结构体**故意不加** `deny_unknown_fields`（配置文件需要向前兼容，老版本读到新配置不能直接失败）。代价是 patch 里拼错的键被合并进去、反序列化时被忽略、保存时消失，**命令返回 `Ok`**，前端 `dispatchConfPatch` 认为保存成功，UI 显示「已保存」但值没变。

不能简单给配置结构体加 `deny_unknown_fields`——那会让旧版本读到新版本写出的配置直接加载失败，破坏向前兼容。正确做法是在 `amend()` 里做「已知键集合」校验：白名单从静态字段清单（或 `Default` 实例的序列化结果）取，并显式补上迁移别名；patch 顶层出现集合外的键就返回 `Err`，未知键在 `from_str` 之前就被拦住。不要直接用当前实例的 `serde_json::to_value(&self)`，原因见下方修法第 1 条。

**现象二：同一配置组读写命名方向相反**。

| 方向 | 通道 | 命名 |
|---|---|---|
| 读 | `get_app_conf` / `get_orb_conf` / `get_mascot_conf` / `get_vault_conf` → `*ConfDto` | `rename_all = "camelCase"`，线格式 camelCase |
| 写 | `patch_conf` → `serde_json::Value` → `amend()` | 配置结构体无 `rename_all`，线格式 snake_case；前端靠 `src/services/settingService.ts::patchToSnakeCase` 逐键转 |

`patchToSnakeCase` 只转顶层键，且对已经是 snake_case 的键原样保留——这条「刻意」既没有注释也没有测试。它与现象一叠加后形成同一种失败模式：命名转换一旦和后端不一致，保存会静默变成空操作。

**修法**：

1. `assistant-config` 四个 `amend()` 统一加顶层键白名单校验，未知键 `Err`（错误信息里带键名）。白名单从 `Default`/静态字段清单派生，不要用当前实例的 `serde_json::to_value(&self)`：`legacy_compact_threshold_percent` 带 `skip_serializing_if`，会让白名单随实例状态出现/消失；同时显式允许迁移别名 `compact_threshold_percent`；
2. 改成 `Err` 会与既有测试冲突：`assistant-config/tests/app_conf.rs::amend_unknown_key_ignored` 与 `assistant-config/tests/vault_conf.rs::amend_unknown_key_ignored` 当前把「忽略未知键」固化为预期行为，必须同步改为断言报错；
3. `patchToSnakeCase` 补注释说明「仅顶层、`keybindings` 内层不转」的理由，并补一个最小单测（含一个多词字段与一个已是 snake_case 的键）；
4. 可选（成本更高）：`patch_conf` 入参改为按 group 反序列化成对应 `*ConfDto`（camelCase + `deny_unknown_fields`），后端再转内部结构体，读写命名就此统一。此项会动到 `patch_conf` 的签名与四个 `apply_*_conf`，建议单独立项；完成后 `patchToSnakeCase` 成为死代码，应删除（见 C7）。

**验证**：`cargo test -p assistant-config`（新增 amend 未知键用例）；`npm run test`（`settingService` 命名转换用例）。

---

### C4 配置 DTO 与配置结构体的字段漂移无测试（优先级 P2）

**现象**：`AppConfDto` / `OrbConfDto` / `MascotConfDto` / `VaultConfDto` 与 `assistant-config` 中对应结构体之间是**手写 `From` 逐字段映射**，没有编译期约束，也没有任何测试。`src-tauri/src/protocol/dto/config.rs` 下 `#[test]` 检索为 0 处。

给 `AppConf` 加一个字段、忘记加进 `AppConfDto` 的后果：配置能落盘、能读到，但前端永远看不到该字段，设置页对应项渲染为空，且没有任何报错。

**为什么不能直接写「字段集合相等」测试**：有两个约束必须先处理。

- 配置结构体没有 `rename_all`，序列化键是 snake_case；`*ConfDto` 是 camelCase。直接把 `serde_json::to_value(&conf)` 与 DTO 序列化结果的键集合比较，所有多词字段都会不等，必须先做 snake→camel 归一化；
- `AppConf::legacy_compact_threshold_percent`（`#[serde(rename = "compact_threshold_percent")]`、`skip_serializing_if = "Option::is_none"`）是迁移专用字段：`None` 时键不出现、`Some` 时以旧键名出现，键集合会随实例状态变化。排除清单必须静态写死，不能用实例序列化结果推导。

**修法**：

- 在 `src-tauri/tests/protocol_dto_roundtrip.rs` 中，用各结构体的构造入口（如 `AppConf::new()`）或显式全字段构造，序列化后做 snake→camel 归一化，再与 `*ConfDto::from(conf)` 序列化结果的键集合比对；排除清单只允许 `compact_threshold_percent` 一项并注释迁移用途。**不要放 lib 的 `#[cfg(test)]`**：Windows 上 `assistant-app` lib 测试二进制缺 Common Controls v6 清单，启动即 `STATUS_ENTRYPOINT_NOT_FOUND`，lib 单测在本机不可执行；
- DTO 的 camelCase 输出断言放 `src-tauri/tests/protocol_contract.rs`（已有 `conversation_dto_outputs_camel_case` 等先例）。`src-tauri/tests/protocol_bindings.rs` 是契约生成器，会重写 `src/types/protocol.contract.ts`，不要往里加 serde 断言。

**验证**：`cargo test -p assistant-app --test protocol_dto_roundtrip --test protocol_contract`；临时删 `AppConfDto` 一个字段确认测试失败，再恢复。`--test protocol_bindings` 不执行任何断言，不能作为本项的验证命令。

---

### C5 服务层手写 DTO 副本（优先级 P3）

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

**验证**：`npm run typecheck`；删掉生成契约里某个字段，确认所有消费点编译失败（即证明类型真的连上了）。

---

### C6 错误通道不对称：chat 域仍是 `Result<_, String>`（优先级 P3，结构性）

**现象**：`src-tauri/src/cmd` 下 16 个文件共 69 处 `Result<_, String>`（含内部 helper），其中 chat / conversation / provider / storage / config 域的命令全部是纯字符串错误；只有 vault 域（`src-tauri/src/cmd/vault/sync.rs` 等）返回结构化 `VaultErrorDto`。

**影响**：同一类失败在两条通道上表现不同。

| 通道 | 结构 | 前端可用的信息 |
|---|---|---|
| 流内错误 | `ProviderErrorPayloadDto` | `code` / `retryable` / `retryAfterMs` / `httpStatus` / `provider` |
| 命令级错误（chat 域） | `String` | 只有文案；`toChatError` 归一成 `InternalError`，失去重试倒计时与分类 |

即「发送消息」本身失败（如 provider 限流、鉴权失败）与「流中断」失败，在 UI 上的呈现和可操作性不一致。

**修法**：定一个通用 `CommandErrorDto`（`code` / `message` / `retryable` / `retryAfterMs` / `details`），新命令按新契约写；存量命令不必一次性改完——前端 `toChatError` 已同时接受 `string` 与结构化对象，可以逐个迁移而不破坏调用点。**建议单独立项**，因为它会触及 `#[command]` 签名、前端 chat / conversation 等域的调用点（如 `src/services/chatService.ts`）返回类型与错误分支。

**验证**：迁移单个命令（如 `send_chat_message`）后，确认前端仍走 `toChatError` 且能拿到 `retryAfterMs`。

---

### C7 遗留死代码与一次性兼容垫片（优先级 P3）

**现象**：几轮契约迁移留下了不再被消费的导出、别名和临时转换层。它们不报错、不影响运行，但会让下一次审计误以为仍是有效入口，也会在「统一命名」时被误改。以下清单已逐一核对消费点。

| 位置 | 遗留物 | 判定证据 | 处理 |
|---|---|---|---|
| `src/vault/core/vault.types.ts` | `EntryPreviewField` / `PendingRemovalEntry` / `TotpInput` / `UnreadableReason` 四个别名 | barrel 之外引用数为 0；对应原类型仍由 `export *` 导出 | 删除这 4 行别名；其余 22 个别名仍有消费点，暂不能整体删除 barrel |
| `src/services/settingService.ts` | `InstallAssetResult` | C5 改为生成类型后成为重复副本 | 随 C5 一并删除，不留 `export type` 别名 |
| `src/services/storageService.ts` | `AttachmentSummary` / `ConversationAttachmentSummary` / `StorageBreakdownDto` / `AttachmentDeleteResultDto` 四个手写接口 | 与生成契约同名同构 | 随 C5 一并删除 |
| `src/services/orbService.ts` | `MousePosition` / `ScreenSize` | 对应 `MousePositionDto` / `ScreenSizeDto` | 随 C5 一并删除 |
| `src/types/events.ts` | `ThemeChangedPayload` 的 `Omit` 窄化 | C1 修法 A 后 `resolved` 已是封闭枚举 | 随 C1 一并删除 |
| `src/services/settingService.ts` | `patchToSnakeCase` | 仅 C3 修法 4（`patch_conf` 入参改为 camelCase DTO）完成后失效 | 届时删除，并移除对应单测 |

**明确不列为死代码**（避免后续审计重复挖）：

- `assistant-config::app_conf::AppConf::legacy_compact_threshold_percent`：仍有旧配置迁移读取用途，等旧版本兼容窗口关闭后另立迁移项；
- `assistant-storage` 的 `messages.tool_calls_json`：DB 预留列（当前写入恒 NULL，`db.rs` 有「禁止 DROP」注释），不是 wire 字段残留。

**验证**：`npm run typecheck`、`npm run lint`；删除别名后全局检索无残留引用；多删一项会直接编译失败，可作为反向验证。

---

## 4. 有意例外与待办事项

| 项 | 位置 | 现状 | 处理 |
|---|---|---|---|
| `role` / `titleSource` 为开放字符串 | `MessageDto::role`、`ConversationDto::title_source` | DB 列为 TEXT，加枚举需要数据迁移；前端按 `user` / `assistant` / `compact_summary` / `placeholder` 比较，未知值走非用户分支渲染，不会崩 | 保留。在字段处补注释「存储边界例外，值域由 DB 决定，前端必须兜底」 |
| `ContentBlockDto::tool_use_id` snake_case | `src-tauri/src/protocol/dto/chat.rs::ContentBlockDto` 的 `ToolUse` / `ToolResult` | 全仓唯一 snake_case 线格式字段（`#[serde(rename = "tool_use_id")]`），内嵌工具协议原文；前端消费点为 0 | 保留。在 `#[serde(rename)]` 处补注释「协议原文，禁止改为 camelCase」，否则后续统一命名时会被误改并破坏与工具协议的对应关系 |
| `ApiFormat` 前端联合 | `src/settings/provider/types.ts::ApiFormat` 为 `"openai" \| "anthropic" \| "local"` | 后端 `ProviderInputDto::type` 是 `String`，格式注册表也是开放集合；`providerLookup.ts` 用 `as ApiFormat` 强转，`getApiFormatByType` 对未知类型回落 `"openai"` | 类型保留；`as ApiFormat` 的静默错标仍需修（待办）：当前未知格式会被**错标**成 OpenAI（图标、是否需要 API Key、默认 Base URL 全部走 OpenAI 分支），且无任何日志；改为运行时校验 + 显式回落日志 |

---

## 5. 处理顺序与验证清单

建议顺序：C1（行为正确性，含 C7 中对应删除）→ C2（防回归，纯类型层）→ C5（去重，含 C7 中对应删除）→ C3（含 C7 中对应删除）→ C4（后端校验与测试）→ C7（死代码收尾）→ C6（另立项）。

| 阶段 | 内容 | 验证命令（在仓库根目录执行；cargo 在 `src-tauri` 目录执行） |
|---|---|---|
| 1 | C1 主题解析与 DTO 收窄 | `cargo test -p assistant-app --test protocol_dto_roundtrip`；`npm run typecheck`；`npm run test` |
| 2 | C2 错误码映射表 | `npm run typecheck`；`npm run test` |
| 3 | C5 服务层复用生成类型 | `npm run typecheck`；`npm run lint` |
| 4 | C3 `amend()` 未知键校验 + 命名转换注释与用例 | `cargo test -p assistant-config`；`npm run test` |
| 5 | C4 配置 DTO 字段集合测试 | `cargo test -p assistant-app --test protocol_dto_roundtrip`；`cargo test -p assistant-app --test protocol_contract` |
| 6 | C7 死代码与兼容垫片清理 | `npm run typecheck`；`npm run lint`；全局检索无残留引用 |
| 7 | C6 命令错误契约（另立项） | 视迁移范围 |

通用收尾：`npm run contract:check`（确认生成契约与源码一致、无遗漏提交）、`npm run lint`、`npm run typecheck`、`npm run test`；并逐项确认 C7 表中的遗留物已删除，或已在「明确不列为死代码」中写明保留原因。

---

## 6. 定位锚点

行号会漂移，优先按以下锚点检索：

| 主题 | 检索词 |
|---|---|
| C1 | `ThemeChangedPayloadDto`、`THEME_CHANGED`、`dispatchThemeChanged`、`win.onThemeChanged`、`data-theme` |
| C2 | `ChatErrorCode`、`ProviderErrorCodeDto`、`VAULT_ERROR_CODES`、`VaultErrorCode` |
| C3 | `fn amend`、`patchToSnakeCase`、`patch_conf` |
| C4 | `impl From<assistant_config::`、`AppConfDto` |
| C5 | `InstallAssetResult`、`StorageBreakdownDto`、`MousePositionDto` |
| C6 | `Result<(), String>`、`VaultErrorDto`、`toChatError` |
| C7 | `EntryPreviewField`、`PendingRemovalEntry`、`TotpInput`、`UnreadableReason`、`InstallAssetResult`、`patchToSnakeCase`、`legacy_compact_threshold_percent`、`tool_calls_json` |
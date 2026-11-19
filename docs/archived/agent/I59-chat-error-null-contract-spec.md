# I59 — 前端 `ChatError` 守卫与 wire `null` 语义对齐修复规格

| 项 | 值 |
|---|---|
| 阶段 | **缺陷修复**（B 级：错误展示失真，可归因性丢失）；切片 T1–T4 全部在本阶段内（§11） |
| 状态 | 已完成（T1–T4 落地，见 §15）；同类判据与同类文档已合并（§14） |
| 日期 | 2026-11-19 |
| 引入点 | `ddca9bb`（2026-11-03 20:19，`feat(dto): 协议契约生成与语义统一`）——该提交移除输出 DTO 的 `skip_serializing_if`、统一为「键恒在、值 `null`」，但**未同步放宽前端守卫** |
| 关联 | I16 frontend-backend protocol spec（已归档）（契约生成的起点与 `null` 语义的来源）、I19 error-system-spec（已归档）（错误边界与日志系统）、I38d agent-frontend-modes-spec（已归档）（phase 由后端驱动，本文不改）、I55（观测面只读副本，不受本文影响）、dto-frontend-backend-sync-audit（已归档）（同一漂移的前史：声明 `\| null` 而运行时收 `undefined`）、command-error-contract-spec（已归档）（`toChatError` 为归一化单点）、compat-tech-debt-audit-1（已归档）（守卫过窄 → 静默退化 `InternalError` 的故障模式）、[PROJECT-DESIGN-GUIDELINES](../../analysis/PROJECT-DESIGN-GUIDELINES.md)（「输出 DTO `Option` 不加 `skip_serializing_if`」为项目级硬规则） |
| 影响范围 | `src/chat/core/chatErrors.ts`（唯一生产改动）；新增 `src/chat/core/__tests__/chatErrors.test.ts`；`src/services/__tests__/turnOrchestrator.test.ts` 增一条断言。**零改动**：后端 DTO / `src/types/protocol.contract.ts` / `src/services/stream/*` / UI 组件 |

> 一句话：流内 provider 错误的 wire 载荷自 `ddca9bb` 起把 `httpStatus` / `provider` / `providerCode` 三个可选字段改成**键恒在、值 `null`**，而前端类型守卫 `isChatError` 仍只接受 `undefined`。于是**凡是不带 HTTP 元信息的 provider 错误**（流空闲超时、SSE 断流、网络失败等）在守卫处判定失败，被 `toChatError` 兜底成 `InternalError` + 「未知错误」，**后端已分类的 `Timeout` 与原始文案被丢弃**。本文把守卫判据与生成契约对齐，并用最小样本把该对齐固化成测试。

**编号约定**：`N`（现状事实）、`G`（门禁）、`T`（切片）、`L`（已知限制）、`R`（拒绝方案）、`OQ`（未决问题）在本文内独立编号；跨文档引用带文档前缀（`I16 §…`）。裸写 `§` 一律指本文。

---

## §0 范围与切分轴

### 0.1 交付范围

| 项 | 内容 |
|---|---|
| **纳入** | ① `isChatError` 的 `httpStatus` / `provider` / `providerCode` 判据改为「`undefined` 或 `null` 或正确类型」；② `details` 判据同样接受 `null`（防御同类漂移，OQ2）；③ `ChatError` 接口声明与生成契约对齐（`number \| null` / `string \| null`）；④ 新增 `chatErrors` 单测：`null` 载荷的 `Timeout` 必须保留 `code` / `message` / `retryable` / `category`；⑤ `turnOrchestrator` 增一条路由断言，证明 `handleDeltaEvent` 把该载荷转成 `Timeout` 而非 `InternalError` |
| **排除** | 后端序列化策略、生成契约文件、错误 UI 组件、重试按钮逻辑、错误文案本地化、`FeatureErrorBoundary` 语义、任何运行时 schema 校验库 |

### 0.2 切分轴

主判据：**缺陷在「守卫」还是「兜底」**。

| 侧 | 判据 | 归属 |
|---|---|---|
| A（本文） | 输入本已是合法 `ChatError`，只是判据过窄导致误判 → 修**判据** | 本阶段做 |
| B（另立规格） | 输入确实不是 `ChatError`（无 `code` / 无 `retryable`），兜底行为本身需要改 | 不在本文 |

本文**只修判据**，不动兜底分支的返回值语义（§3.4）。理由：兜底分支是「无法判别」时的显式降级入口，放宽它等于放弃类型守卫（§12 R2）。

### 0.3 前置事实校验

修复前必须确认以下三条同时成立，否则本文的归因不成立：

1. 后端 `ProviderErrorPayloadDto` 的三个字段**无** `skip_serializing_if`（§1 N1）；
2. 生成契约声明为 `number | null` / `string | null`（§1 N2）；
3. 触发链路上的错误确实来自 `ProviderErrorPayload::from(&ProviderError)`（三字段 `None`），而不是带 HTTP 元信息的分支（§1 N7）。

三条均由 §1 的代码事实与 §10 的 G1 复现样本兜住。

---

## §1 现状（代码事实）

以下为 2026-11-19 的实际实现，路径相对仓库根。

| 编号 | 事实 | 位置 |
|---|---|---|
| N1 | `ProviderErrorPayloadDto` 的 `http_status` / `provider` / `provider_code` 均为 `Option`，**无** `skip_serializing_if`，序列化为键恒在、值 `null` | `src-tauri/src/protocol/dto/chat/event.rs`（`ProviderErrorPayloadDto`） |
| N2 | 生成契约声明 `httpStatus: number \| null`、`provider: string \| null`、`providerCode: string \| null` | `src/types/protocol.contract.ts`（`ProviderErrorPayloadDto`） |
| N3 | `isChatError` 三个字段的判据是 `=== undefined \|\| typeof === "number"` / `"string"`，**不接受 `null`** | `src/chat/core/chatErrors.ts`（`isChatError`） |
| N4 | `details` 判据要求 `typeof === "object" && !== null`，同样不接受 `null` | 同上 |
| N5 | `ChatError` 接口把三个字段声明为可选非空（`httpStatus?: number` 等），与 N2 的 `\| null` 不一致 | 同上 |
| N6 | `toChatError` 在 `isChatError` 失败后，对象输入走最后兜底：`InternalError` + `translateKey("chat.error.unknown_error")`，**原 `message` 丢弃** | 同上（`toChatError`） |
| N7 | 流空闲超时分支构造 `ProviderError::Timeout(...)` 后直接 `yield Err`，**不**重试；其载荷经 `ProviderErrorPayload::from(&ProviderError)` 构造，三字段全 `None` | `src-tauri/crates/assistant-providers/src/openai/chat.rs`（`StreamTimeoutStage::Idle` 分支）、`src-tauri/crates/assistant-providers/src/base.rs`（`From<&ProviderError>`） |
| N8 | 终态时 `Terminated` 分支先发 `CHAT_DELTA`（`error` 字段非空）再发 `session-state-changed(error)` | `src-tauri/src/adapters/event_sink.rs`（`TurnEvent::Terminated`） |
| N9 | 前端 `chat-delta` 的 `error` 经 `transport` 透传 → `turnOrchestrator.handleDeltaEvent` → `strategy.onDeltaError(toChatError(payload.error))` → `state.errorStream(error)` | `src/services/stream/transport.ts`、`src/services/stream/turnOrchestrator.ts`、`src/services/stream/strategies/{chatTurn,agentTurn}.ts`、`src/stores/chat/chatStorePorts.ts` |
| N10 | `session-state-changed` 的 `error` 只驱动 phase（`stateToPhase` → `"error"`），**不承载气泡文案**；气泡错误只来自 N9 路径 | `src/services/stream/phase.ts`、`src/services/stream/turnOrchestrator.ts`（`onSessionState`） |
| N11 | `isChatError` 的另一个消费方是 `FeatureErrorBoundary`（用于把已分类错误排除出 crash 上报）；判据过窄会让部分已分类错误被当作未分类 | `src/components/FeatureErrorBoundary.tsx` |
| N12 | 仓库内**没有** `chatErrors` 的单测；`protocolContract.test.ts` 未断言 `ProviderErrorPayloadDto` 的 `null` 键形态 | `src/chat/core/__tests__/` |
| N13 | 引入时间线：`6226039`（2026-10-17）写下守卫，当时后端三个字段带 `skip_serializing_if = "Option::is_none"`，`None` 时**键被省略**，前端看到 `undefined`，判据成立；`ddca9bb`（2026-11-03）移除 `skip_serializing_if` 后判据失配 | git 历史（`git log -S` 可复核） |

**现场证据**（`%LOCALAPPDATA%\com.assistant\logs\assistant-dev.log`，2026-11-19）：

```
10:38:23 ERROR assistant_providers::sse_log  SSE error: stream idle timeout
10:38:23 WARN  assistant_agent::loop_executor::call  provider 失败重试 1/1 … err=Request timeout: No chunk received for 30s (stream idle)
10:39:13 ERROR assistant_providers::sse_log  SSE error: stream idle timeout
10:39:13 INFO  assistant_app::adapters::event_sink  chat-stream-end … status=failed … error=Timeout: Request timeout: No chunk received for 30s (stream idle)
10:39:13 WARN  webview … [chatStorePorts] chat-error code=InternalError retryable=false message=未知错误
```

第 4 行与第 5 行是同一次终态：**后端已判定 `Timeout`，前端记录成 `InternalError`**。`turn` 统计 `iterations=1 model_calls=2 elapsed_ms=83067` 说明是两次 30s 空闲超时（第一次超时后 agent 层重试一次，第二次终态失败），与 N7 的「Idle 不内部重试」一致。

---

## §2 目标与非目标

### 2.1 目标

1. **可归因**：流内 provider 错误的 `code` / `message` / `retryable` 原样到达 UI 与日志，不再退化为 `InternalError` / 「未知错误」。
2. **同源**：守卫判据与生成契约的 `null` 语义一致，且该一致性由测试固定，而不是靠人工复核。
3. **零回归**：合法 `ChatError` 的判定结果、非法输入的兜底结果、字符串输入的 `message` 保留行为均不变。

### 2.2 非目标

- 不改后端 `ProviderErrorPayloadDto`（输出 DTO `Option` 不加 `skip_serializing_if` 是项目规则）。
- 不改生成契约 `src/types/protocol.contract.ts`。
- 不做 provider `message` 的中文本地化（§9 L1、§13 OQ3）。
- 不改 `errorStream` / `setErrorOnMessage` 的存储与 UI 行为（含「无 `streamingMessageId` 时静默返回」，§9 L4、§13 OQ4）。
- 不改 `FeatureErrorBoundary` 的判定语义（判据变宽后其行为只朝「更少误报 crash」方向变化）。
- 不引入 `zod` / `valibot` 等运行时 schema 校验（§12 R3）。

---

## §3 契约

### 3.1 wire 语义（唯一来源）

`ProviderErrorPayloadDto`（输出 DTO，`deny_unknown_fields`）：

| 字段 | wire 类型 | 必填 | 语义 |
|---|---|---|---|
| `code` | `CommandErrorCodeDto`（PascalCase 字符串） | 是 | 错误码唯一来源，前端不校验枚举（N3 的既有设计，保留） |
| `message` | `string` | 是 | 后端已构造的文案；流错误恒非空 |
| `retryAfterMs` | `number \| null` | 是（键恒在） | 仅限流时有值 |
| `retryable` | `boolean` | 是 | 由码推导，不由调用点自由传入 |
| `httpStatus` | `number \| null` | 是（键恒在） | 命令级 / 非 HTTP 错误为 `null` |
| `provider` | `string \| null` | 是（键恒在） | 同上 |
| `providerCode` | `string \| null` | 是（键恒在） | 同上 |

**本文不修改这张表**，只让前端守卫与它一致。

### 3.2 `ChatError` 类型声明（必须与契约对齐）

| 字段 | 现状（N5） | 目标 |
|---|---|---|
| `code` | `ChatErrorCode` | 不变 |
| `message` | `string`（可选） | 不变（本地兜底路径可缺省，wire 路径恒有） |
| `retryAfterMs` | `number \| null` | 不变 |
| `retryable` | `boolean` | 不变 |
| `httpStatus` | `number`（可选） | `number \| null`（可选） |
| `provider` | `string`（可选） | `string \| null`（可选） |
| `providerCode` | `string`（可选） | `string \| null`（可选） |
| `category` | `ChatErrorCategory`（可选） | 不变（本地派生，wire 不承载） |
| `details` | `Record<string, unknown>`（可选） | 不变（本地注入，wire 不承载） |

`retryAfterMs` 已是 `number \| null`，无需改。`details` 的 wire 侧不存在，故不引入 `\| null` 到类型上，只在判据上放宽（§3.3）。

### 3.3 守卫判据表

`isChatError` 逐字段的**可接受形态**：

| 字段 | 可接受 | 拒绝 |
|---|---|---|
| `code` | `string`（不校验是否在枚举内，N3 的既有设计） | 非字符串 / 缺失 |
| `message` | `undefined` / `string` | 其它类型 |
| `retryAfterMs` | `undefined` / `null` / `number` | 其它类型 |
| `retryable` | `boolean`（必填） | 缺失 / 非布尔 |
| `httpStatus` | `undefined` / `null` / `number` | 其它类型 |
| `provider` | `undefined` / `null` / `string` | 其它类型 |
| `providerCode` | `undefined` / `null` / `string` | 其它类型 |
| `details` | `undefined` / `null` / 非 `null` 对象 | 非对象 / 数组以外的原始值（保持现状的「对象」判据，仅增加 `null` 放行） |

实现约束：**禁止**把判据写成 `typeof obj.x !== "undefined"` 之类的反向判断；必须保持「`undefined` / `null` 都放行，其余要求精确类型」的对称形式，避免下一轮契约变更再次漏项。

### 3.4 `toChatError` 保持不变的语义

1. `isChatError` 通过时：`details` 与 `extra` 合并，`category` 由 `inferCategory` 派生；`retryAfterMs` 缺省补 `null`。
2. 字符串输入：保留 `message`，`code = InternalError`，`retryable = false`，`category = Unknown`。
3. 其它输入：`InternalError` + `translateKey("chat.error.unknown_error")`。

本文**只改 1 的入口判据**，不改 2 / 3 的返回值。

### 3.5 最小复现样本（测试即契约）

```ts
const timeoutPayload = {
  code: "Timeout",
  message: "Request timeout: No chunk received for 30s (stream idle)",
  retryAfterMs: null,
  retryable: true,
  httpStatus: null,
  provider: null,
  providerCode: null,
} as const;
```

断言：`toChatError(timeoutPayload)` 返回 `code = "Timeout"`、`message` 与输入逐字相同、`retryable = true`、`category = "timeout"`，且**不**包含 `chat.error.unknown_error` 的文案。

该样本必须与 `src/types/protocol.contract.ts` 的 `ProviderErrorPayloadDto` 形状一致；若将来契约新增字段，G5 的编译期断言会先失败。

---

## §4 行为

### 4.1 修复前后对照（同一次 `Terminated` 事件）

| 环节 | 修复前 | 修复后 |
|---|---|---|
| `isChatError(payload.error)` | `false`（三个 `null` 字段拒绝） | `true` |
| `toChatError` 结果 | `InternalError` / 「未知错误」/ `retryable = false` | `Timeout` / 后端原文 / `retryable = true` |
| `errorStream` 挂到消息 | 气泡显示「未知错误」 | 气泡显示 `Request timeout: No chunk received for 30s (stream idle)` |
| 日志 `chat-error` | `code=InternalError retryable=false message=未知错误` | `code=Timeout retryable=true message=Request timeout: …` |
| 观测 / phase | `error`（不变） | `error`（不变，N10） |

### 4.2 判定矩阵

| 输入 | 修复前 | 修复后 |
|---|---|---|
| §3.5 的 `Timeout` 全 `null` 载荷 | 兜底 `InternalError` | `Timeout`，原样保留 |
| 带 `httpStatus: 429` 的 `RateLimited` 载荷 | 通过 | 通过（不变） |
| 带 `provider: "openai"` 的 `AuthFailed` 载荷 | 通过 | 通过（不变） |
| `{ code: "X", retryable: true }`（无 `message`） | 通过（`message` 可选） | 通过（不变） |
| `{ code: "X" }`（缺 `retryable`） | 兜底 | 兜底（不变） |
| `"boom"`（字符串） | `InternalError` + `message = "boom"` | 不变 |
| `null` / `{}` / 数字 | 兜底「未知错误」 | 不变 |
| `details: null` 的载荷 | 兜底（N4） | 通过（`details` 合并为空） |

---

## §5 边界情形与失败矩阵

| 编号 | 情形 | 期望 |
|---|---|---|
| E1 | `payload.error` 为 `null`（普通 delta） | 不进入守卫（`if (payload.error)` 前置判空），行为不变 |
| E2 | `attemptId` 与活跃 turn 不匹配（迟到事件） | `handleDeltaEvent` 提前 return，不展示错误（既有行为，不变） |
| E3 | 流错误先到、`session-state error` 后到 | 气泡错误以 delta 路径为准；`session-state` 只改 phase 并 `cancelStreaming`，不覆盖文案（N10，不变） |
| E4 | `errorStream` 时 `streamingMessageId` 为空 | 静默返回，错误丢失（§9 L4，本文不修，OQ4） |
| E5 | 消息上已有 `error` | `errorStream` 直接 `cancelStreaming`，不覆盖（既有行为，不变） |
| E6 | `code` 不在前端枚举内（契约漏跑） | 守卫仍通过，`category = Unknown`，`message` 原样（既有设计，不变） |

---

## §6 文案

| 编号 | 项 | 规则 |
|---|---|---|
| C1 | provider 流错误文案 | **直出后端 `message`**，不做前端改写；`retryAfterMs > 0` 时保留既有的 `(retry after Ns)` 后缀（`errorStream` 既有逻辑） |
| C2 | 兜底文案 | `chat.error.unknown_error`（中「未知错误」/ 英「Unknown error」）**保留**，仅用于真正无法判别的输入 |
| C3 | 日志格式 | 保持 `chat-error code=<code> retryable=<bool> message=<msg>`；修复后 `code` 应为 `Timeout` 等真实码 |
| C4 | 断言 | 任何新增文案不得出现在 §3.5 样本的输出中（G1） |

---

## §7 与相邻组件

| 组件 | 关系 | 本文是否改动 |
|---|---|---|
| `ProviderErrorPayloadDto` / 生成契约 | 唯一 wire 来源 | 否 |
| `transport.ts` | 只透传 `error`，不做判定 | 否 |
| `turnOrchestrator.handleDeltaEvent` | 调用 `toChatError` 的入口 | 否（仅加断言） |
| `chatTurn` / `agentTurn` 策略 | `onDeltaError` 调 `errorStream` | 否 |
| `chatStorePorts.errorStream` / `setErrorOnMessage` | 挂错到消息并记日志 | 否 |
| `FeatureErrorBoundary` | `isChatError` 第二消费方；判据变宽只减少误报 crash | 否 |
| `ApprovalCard` / `QuestionCard` | `toChatError(err).message ?? ""` | 否（修复后能取到真实文案） |
| `phase.ts` | `SessionStateDto` → phase | 否 |

---

## §8 验收

- 新增断言落在 `src/chat/core/__tests__/chatErrors.test.ts`（T2）与 `src/services/__tests__/turnOrchestrator.test.ts`（T2 的一条路由断言）。
- 每条门禁（§10）至少对应一条断言；无断言的项视为未交付。唯一例外是 G9：判据形态是静态代码事实，由 §14.1 的全仓检索人工核对，不做运行期断言。
- 回归口径：`npm run typecheck`、`npm run lint`、`npm test`（cwd = 仓库根）。后端零改动，不需要 `cargo test`。

---

## §9 已知限制（边界，不是缺陷）

| 编号 | 限制 | 理由 | 逃生路径 |
|---|---|---|---|
| L1 | provider 错误的 `message` 是英文（后端 `ProviderError` 的 `Display`） | 后端未做错误文案本地化，前端直出后端文案（C1） | 若需中文，另立规格做「码 → i18n 键」映射（OQ3） |
| L2 | 守卫不校验 `code` 是否在枚举内 | 契约漏跑 / 手工改生成物时仍要保留后端文案，分类回落 `Unknown`（既有设计） | — |
| L3 | 守卫不拒绝未知字段 | 输出 DTO 在 Rust 侧已 `deny_unknown_fields`，前端无需重复校验 | — |
| L4 | `errorStream` 在无 `streamingMessageId` 时静默丢弃错误 | 该路径需要「无消息可挂」的 UI 设计（全局 toast？），超出本文 | 另立规格（OQ4） |
| L5 | `ChatError` 仍是手写接口，可能与生成契约再次漂移 | 本文只对齐当前三字段；彻底消除需类型派生（OQ1） | OQ1 落地后改为 `ProviderErrorPayloadDto & { category?, details? }` |

---

## §10 门禁

| 编号 | 门禁 | 判据 |
|---|---|---|
| G1 | 主缺陷修复 | §3.5 样本 → `toChatError` 得 `code = "Timeout"`、`message` 逐字一致、`retryable = true`、`category = "timeout"`，且不含 `unknown_error` 文案 |
| G2 | 正样本零回归 | 带 `httpStatus: 429` / `provider: "openai"` 的合法载荷仍通过，字段逐字段保留 |
| G3 | 兜底零回归 | `{}` / `null` / `123` 仍得 `InternalError` + 「未知错误」；字符串输入仍保留 `message` 且 `code = InternalError` |
| G4 | `details: null` 放行 | `details: null` 的载荷通过守卫；`toChatError` 不因此造出空 `details`（保持 `undefined`） |
| G5 | 类型对齐 | `ChatError` 三字段声明为 `T \| null`；`npm run typecheck` 通过；新增测试中的样本字面量可直接赋给 `ProviderErrorPayloadDto`（编译期兜住契约漂移） |
| G6 | 路由透传 | `turnOrchestrator.handleDeltaEvent` 收到 §3.5 载荷时调用 `onDeltaError`，且传入对象的 `code === "Timeout"`（不是 `InternalError`） |
| G7 | 零后端改动 | `git diff` 不含 `src-tauri/**` 与 `src/types/protocol.contract.ts`；`npm run contract:gen` 无 diff |
| G8 | 消费方不回归 | `FeatureErrorBoundary` 对普通 `Error` 仍判为非 `ChatError`（不因判据变宽而吞掉 crash 上报） |
| G9 | 同类判据零残留 | 全仓检索「`: value is` 守卫」与「`=== undefined \|\|` 判据」后，把 wire 可空字段写成 `undefined` 单侧判据的地方**仅剩零处**（§14.1 逐项核对） |

---

## §11 切片

| 编号 | 切片 | 交付物 | 依赖 |
|---|---|---|---|
| T1 | 类型与守卫对齐 | `ChatError` 三字段改 `T \| null`；`isChatError` 的 `httpStatus` / `provider` / `providerCode` / `details` 判据接受 `null`；G5 / G7 | — |
| T2 | 回归测试 | 新增 `src/chat/core/__tests__/chatErrors.test.ts`（G1–G4、G8）；`turnOrchestrator.test.ts` 增一条路由断言（G6） | T1 |
| T3 | 门禁核对 | `npm run typecheck` / `npm run lint` / `npm test` 全绿；人工复核 `git diff` 仅含 T1 / T2 文件 | T2 |
| T4 | 同类文档合并 | §14：全仓同类守卫 / 判据检索表、U1–U5 通用约束、与三份已归档文档的关系；G9 | T1–T3（结论需建立在已落地的判据上） |

T1 与 T2 是**串行**（测试要引用修复后的判据）；T3 与任何并行开发无冲突；T4 只写本文档，与 T1–T3 的文件完全不重叠（但结论依赖它们已落地，故排在最后）。

---

## §12 拒绝方案

| 编号 | 方案 | 拒绝理由 |
|---|---|---|
| R1 | 后端恢复 `skip_serializing_if = "Option::is_none"` | 违反项目规则「输出 DTO `Option` 键恒在、值 `null`」；且会让前端其它消费点必须回退到 `undefined` 判据，等于把契约倒退回 `ddca9bb` 之前 |
| R2 | 放宽 `toChatError` 兜底：对象只要有 `code` 就当 `ChatError` | 放弃类型守卫；任意 `{ code: "x" }` 会被当作已分类错误，`category` / `retryable` / `message` 都可能错，且 `FeatureErrorBoundary` 会误吞真实 crash |
| R3 | 引入 `zod` / `valibot` 做运行时校验 | 新增依赖；契约已由 `ts-rs` 单源生成，再手写一份 schema 必然漂移，且与项目「类型安全规范」的单一来源原则冲突 |
| R4 | 后端把 `None` 序列化成 `""` / `0` | 伪造语义：`httpStatus = 0` 不是合法 HTTP 状态，`provider = ""` 会误导诊断 |
| R5 | 在 UI 层直接读 `payload.message` | 绕过 `toChatError` 单点，`ApprovalCard` / `QuestionCard` / `FeatureErrorBoundary` 三处消费口径会分裂 |
| R6 | 只改 `ChatError` 类型、不改守卫 | 类型断言让 `null` 通过编译，但运行时判据仍拒，缺陷原样存在 |
| R7 | 把 `null` 转成 `undefined` 再喂给守卫（如 `JSON.parse(JSON.stringify(x))` 之类） | 多一层无意义的拷贝；且 `undefined` 在 wire 上本就不存在，属于把契约语义抹平 |

---

## §13 未决问题

| 编号 | 问题 | 影响 |
|---|---|---|
| OQ1 | `ChatError` 是否改为从生成契约派生（`type ChatError = ProviderErrorPayloadDto & { category?: ChatErrorCategory; details?: Record<string, unknown> }`），彻底消除手写字段漂移 | 若采纳，需同步调整 `isChatError` 的字段表来源；本文暂按手写接口对齐（§9 L5） |
| OQ2 | `details` 判据是否接受 `null` | **已决（采纳）**：纯防御性改动，G4 生效；wire 不承载 `details`，故类型上仍不引入 `\| null`（§3.2） |
| OQ3 | provider `message` 是否做前端本地化（按 `code` 映射 i18n 键） | 影响 `C1`；若做，需登记码 → 键映射表并补 i18n 条目 |
| OQ4 | `errorStream` 在无 `streamingMessageId` 时静默丢弃错误，是否另立规格 | 首 token 前失败的场景会「无任何可见错误」；与本文正交 |

---

## §14 同类判据与同类文档合并

> 本节回答「项目里还有没有同一类问题、同类文档怎么一起解决」：先做全仓检索，再把三份已归档同类文档的共性规则固化为可复用约束（U 编号），并作为 G9 的判据。

### 14.1 全仓同类判据检索（2026-11-19）

检索口径：① `src/**` 内全部 `: value is` 形式的类型守卫；② 全部 `=== undefined ||` 单侧判据；③ wire 可空字段（`httpStatus` / `providerCode` / `retryAfterMs`）的消费点。

| 位置 | 判据形态 | 与契约的关系 | 结论 |
|---|---|---|---|
| `src/chat/core/chatErrors.ts`（`isChatError`） | 逐字段 `undefined \|\| typeof` | 三字段在契约中是 `T \| null` | **缺陷**（N3 / N4），本文修复（T1） |
| `src/vault/core/display/vaultError.ts`（`isVaultErrorDto`） | 只判必填字段：`code`（含枚举校验）+ `message` + `retryable` | 契约中三者均不可空，可空字段不参与判据 | 安全；可作「只判必填字段」的参照范式（U3） |
| `src/mascot/renderers/gif/gifUrlValidator.ts`（`isValidGifSrc`） | 字符串 URL 前缀校验 | 非 wire 数据 | 无关 |
| `src/stores/chat/chatStorePorts.ts`（`errorStream`） | `error.retryAfterMs != null && > 0` | `number \| null` | null-safe，无需改 |
| `src/chat/core/chatErrorRecovery.ts` | `error.httpStatus ? … : null`、`error.providerCode ? … : null` | `T \| null` | null-safe（`null` 与 `undefined` 同为假值） |
| `src/chat/message/groupNodeRows.ts`、`src/audit/detail.ts` | `=== undefined \|\|` | 本地 UI 值（分组游标、可选入参），非 wire | 无关 |

结论：`isChatError` 是**全仓唯一**把 wire 可空字段写成 `undefined` 单侧判据的地方；其余守卫与消费点要么不碰可空字段，要么本身 null-safe。故本次修复不需要扩散到其它文件（G9）。

### 14.2 固化为通用约束（U）

| 编号 | 约束 | 来源 | 落点 |
|---|---|---|---|
| U1 | 输出 DTO 的 `Option` 字段**不加** `skip_serializing_if`：键恒在、值 `null`；前端对应声明为 `T \| null`（必填）或 `?: T \| null`（可选） | [PROJECT-DESIGN-GUIDELINES](../../analysis/PROJECT-DESIGN-GUIDELINES.md)；I16（已归档） | §3.1 / §3.2；R1 |
| U2 | 前端守卫对 wire 可空字段必须「`undefined` / `null` 双放行 + 精确类型校验」，**禁止** `typeof x !== "undefined"` 之类反向判断 | 本文 N3 / N4 的成因 | §3.3 |
| U3 | 守卫只对**必填字段**做「存在 + 类型」校验；可选字段一律放行 `null` 与 `undefined`，不额外收窄 | `isVaultErrorDto` 范式（§14.1） | §3.3 |
| U4 | 一致性由测试固定，不由文档约定：样本字面量必须能赋给生成契约类型（编译期），运行时行为由单测断言 | dto-frontend-backend-sync-audit（已归档）记录的「type lying」前史 | §3.5 / G5 / T2 |
| U5 | 改输出 DTO 的序列化语义（尤其移除 / 增加 `skip_serializing_if`）时，**同一次提交内**必须扫描前端守卫与可空字段消费点 | compat-tech-debt-audit-1（已归档）记录的故障模式；本文引入点 `ddca9bb` 正是漏了这一步 | §1 N13；G9 |

### 14.3 与同类文档的关系

| 文档 | 记录的同一问题 | 本文如何承接 |
|---|---|---|
| dto-frontend-backend-sync-audit（已归档） | 前端手写类型声明 `\| null`、运行时却收到 `undefined`（type lying） | 本文是它的镜像形态：声明 `number`、运行时收到 `null`；U4 由此而来 |
| command-error-contract-spec（已归档） | `toChatError` 是错误归一化单点，禁止 UI 直读 wire 字段 | §3.4 保持兜底语义不变；R5 拒绝绕行 |
| compat-tech-debt-audit-1（已归档） | 后端新增错误码 / 改序列化后，前端守卫过窄 → 静默退化为 `InternalError` | §1 N6 与 §4.1 的对照表；U5 由此而来 |
| I16 frontend-backend protocol spec（已归档） | `ts-rs` 单源生成契约、`null` 语义的来源 | §3.1 直接引用生成契约，不手抄字段表 |

### 14.4 为何不顺手「防御性放宽」其它守卫

`isVaultErrorDto` 的契约字段全部不可空。若给它加 `null` 放行，等于让类型守卫接受契约禁止的形态——那是**把 type lying 换个方向**（U2 的镜像错误）。同理，`chatStorePorts` 的 `!= null` 与 `chatErrorRecovery` 的真值判断本就 null-safe，改成 `!== undefined` 反而引入回归。**判据只跟随契约，不做无契约依据的放宽**（与 R2 拒绝「放宽兜底」是同一条原则）。

---

## §15 验收记录

- **T1 类型与守卫对齐**：`ChatError` 的 `httpStatus` / `provider` / `providerCode` 改为 `number | null` / `string | null`（仍可选），与生成契约 `ProviderErrorPayloadDto` 一致；`isChatError` 的四个判据（`httpStatus` / `provider` / `providerCode` / `details`）改为「`undefined` / `null` 双放行 + 精确类型」的对称写法，未引入反向判断。`toChatError` 零改动（`details` 为 `null` 时因假值不参与合并，天然不造空对象）（G4 / G5 / G7）。
- **T2 回归测试**：新增 `src/chat/core/__tests__/chatErrors.test.ts`，10 条用例覆盖：全 `null` 载荷通过（G1）、键缺失（`undefined`）仍通过、`details: null` 通过（G4）、类型错误的可选字段仍拒绝、缺 `retryable` / 非对象 / `Error` 实例仍拒绝（G3 / G8）、`Timeout` 载荷经 `toChatError` 保留 `code` / `message` / `retryable` / `category` 且不含 `unknown_error` 文案（G1）、HTTP 元信息逐字段保留（G2）、字符串输入保留 `message`（G3）、`null` / `{}` / `123` 仍走兜底（G3）。`src/services/__tests__/turnOrchestrator.test.ts` 的 `handleDeltaEvent — routing` 增一条：载荷可选字段全 `null` 时 `errorStream` 收到 `code = "Timeout"`、`category = "timeout"`、`retryable = true`（G6）。
- **T3 门禁核对**：`npm run typecheck` 通过；三个改动文件经 `eslint --fix` / `prettier --write` 无残留问题；`npm test` 全量 72 个文件、857 用例通过（1 skipped，为既有跳过项）。零后端改动：`src-tauri/**` 与 `src/types/protocol.contract.ts` 均未改动，`contract:gen` 无需重跑（G7）。
- **T4 同类文档合并**：§14 的检索表、U1–U5 通用约束、与三份已归档同类文档的关系；G9 由 §14.1 的逐项核对兜住。
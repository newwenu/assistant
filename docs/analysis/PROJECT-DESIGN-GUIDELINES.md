# 项目设计指导纲要

> 日期：2026-10-23｜性质：**分析指导**｜来源：I1~I20 全阶段
> 拆分与目录判据的唯一来源是 `docs/principle/CODE-SPLIT-PRINCIPLE.md`；审查判据的唯一来源是 `docs/principle/CODE-REVIEW-PRINCIPLE.md`。本文只记录原则在本项目的**应用细节**与**派生索引**，不复述判据、不另立阈值、不含执行计划。

---

## 1 架构分层

### 1.1 前端 IPC 三层

```
Service（IPC 协议）→ Orchestrator（业务编排）→ Store（纯状态）
```

分层判据见 `docs/principle/CODE-REVIEW-PRINCIPLE.md` D4（命令层薄壳、状态层纯状态）；下表只回答「什么情况下 Store 可以直接调 Service」：

| 级别 | 条件 | 要求 | 典型场景 |
|------|------|------|---------|
| 硬规则 | 有流式/异步状态机，或需跨 Store 协调 | 必须走 Orchestrator + Port，Store 禁止直接调 Service | chat send/cancel/delta、流式工具执行 |
| 软规则 | 无流式状态机且无跨 Store 依赖 | 允许 Store 直接调 Service，编排逻辑必须内聚在单个 action 内 | createConversation（load providers → create → switch）、resendMessage |
| 豁免 | 单步 `await service.xxx() → set(...)` 纯 CRUD/加载 | Store 直接调 Service 合规，无需引入 Port | listConversations、loadAppConf、deleteConversation |

### 1.2 半开放类型

半开放类型用 branded string（`string & { __brand }`）而非固定 union；硬编码映射改为数据化规则对象 + 求值逻辑。封闭集合的判据见 D2。

---

## 2 Store 拆分

拆分判据见 `docs/principle/CODE-SPLIT-PRINCIPLE.md` §1（一个文件一个变化原因）与 §2（行数只作初筛、不作判据）；以下只记录 slice pattern 的应用约定，不另立阈值：

**拆分决策**：

| 拆 | 不拆 |
|---|------|
| 字段生命周期独立（流式 vs 持久化） | 拆后跨 slice 读写增加 |
| | 体量极小（<3 字段 + <2 action） |
| | 拆后 envPort 需跨 store 读取 |

**跨域依赖**：Slice A 可读 Slice B（`get().fieldB`，头部注释 `// 跨 slice 读取`），禁止写；触发 action 用 `get().actionB()` 或组装层注入回调。

**命名**：接口 `ModuleDomainSlice`，文件 `moduleDomainSlice.ts`；slice ≥4 放 `slices/` 子目录，<3 平铺 `core/`。

---

## 3 前后端协议

### 3.1 DTO 规范

```rust
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct XxxInputDto {
    pub required: String,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub optional: Option<String>,
}

#[serde(rename_all = "camelCase")]
pub struct XxxOutputDto {
    pub required: String,
    pub optional: Option<String>,
}
```

- 输出 DTO 的 `Option` **不加** `skip_serializing_if`：键恒在、值为 `null`，前端声明 `T | null` 必填。
- 输入 DTO 的 `Option` 加 `#[serde(default)]` + `#[ts(optional = nullable)]`，并加 `deny_unknown_fields` 拒绝未知字段。
- 禁止 `json!({})` 手动构造 event payload
- 禁止前端 `as` 强转 invoke 返回值，改用泛型 `invoke<T>`

字段命名与生成链的完整规则见 `docs/principle/CODE-REVIEW-PRINCIPLE.md` D2 与 `.trae/rules/project_rules.md`。

### 3.2 Event 三分类

| 职能 | emit | 判断 |
|------|------|------|
| 数据变更通知 | `app.emit` 广播 | "数据变了"，所有关心该数据的窗口都应收到 |
| 窗口 UI 控制 | `window.emit` 定向 | "某窗口做 UI 动作"，仅目标窗口响应 |
| 高频独占流 | `window.emit` 定向 | 某窗口独占渲染流 |

覆盖性与幂等要求见 `docs/principle/CODE-REVIEW-PRINCIPLE.md` §7.3。

### 3.3 类型文件

类型与 Service 解耦，独立 `xxx.types.ts` 文件。新模块优先点号分隔（`vault.types.ts` / `vault.service.ts` / `vaultStore.ts` 命名族）。

---

## 4 错误处理

**六层防御**：UI 组件 → ErrorBoundary → 全局处理器 → createLogger → 结构化错误 → Rust crash_dump

**硬规则**：
1. 禁止 `.catch(() => {})` 静默吞没（至少 `log.warn`）
2. 禁止裸 `console.error`（统一 `createLogger(context)`)
3. Store 级错误必须用户可感知（通知系统）
4. React.lazy 必须包裹 ErrorBoundary

错误分类与「必然失败的类别不得提供重试」的判据见 `docs/principle/CODE-REVIEW-PRINCIPLE.md` D3。

**统一重试原语**：所有重试/重发/编辑重发收敛到 `resendFrom(pivotTimestamp, content)`，禁止多条路径。重试必须精确关联出错消息 ID，错误事件需幂等保护（terminal dedup），后端 sleep 必须检查 cancel_token。

---

## 5 流式架构

### 5.1 双层分离

```
StreamAccumulator（累积状态）  — apply_delta() → ApplyDeltaOutcome；into_outcome(self) → StreamOutcome
consume_stream（主循环）       — 纯 async loop，cancel_token.is_cancelled() 检查，自身无业务逻辑
provider SSE 层               — tokio::select! { cancel / next }，cancel 与流读取并发竞争
```

- `into_outcome(self)` 消费 self：防半构造状态误用
- `ApplyDeltaOutcome` 结构化返回：调用方精确响应不同事件类型
- `has_effective_output`：仅用户可见输出（text/tool call）设 true，thinking 不算
- `partial_recovery_reason`：零成本预留，扩展时无需改签名

### 5.2 attempt_id 身份标识

`message_id` = 持久化身份，`attempt_id` = 流事件归属，两者不得混用（判据见 D7「流事件身份」）。

| 要素 | 要求 |
|------|------|
| 生成 | 后端（前端禁止造临时 ID） |
| 传递 | 每帧携带 `attempt_id` |
| start 帧 | 流开始前发 `chat-stream-start`（含 `attempt_id`） |
| 过滤 | 所有事件类型统一按 `attempt_id` 过滤 |
| 兼容 | `attempt_id` optional，旧事件退化为不过滤 |

---

## 6 参考项目速查

| 项目 | 核心借鉴 | 借鉴文档（均已归档） |
|------|---------|---------|
| **OpenBitFun** | Port 接口、Permission Engine、createLogger、lazyWithRecovery、DTO 集中、StreamContext+StreamProcessor、round_executor(attempt_id)、app.emit | I13 chat-service-split-spec、I15 chat-runtime-spec、I16 frontend-backend-protocol-spec、I17 chat-retry-recovery-spec、I18 agent-tool-architecture-spec、I19 code-responsibility |
| **OpenHuman** | AttachmentStash、ToolPolicy trait、BranchPickerPrimitive、app.emit | I2e MULTIMODAL-ATTACHMENTS-ANALYSIS、I17b conversation-branch-spec、I18 agent-tool-architecture-spec、I20 cross-window-event-sync-spec |
| **deepseek-harness-orb** | LlmAttemptId branded type、start/chunk/end 帧、settle/abandon 双路径 | I15 chat-runtime-spec、I16 stream-attempt-identity-spec |
| **AIRI** | ApiError 工厂、image-projection 降级、transient retry | I19 error-system-spec、I2e vision-resolution-design |
| **KeePassXC** | 屏幕锁/生物识别/TOTP/HIBP/SSH Agent | I9 vault-spec-3-security-enhance |
| **Vaultwarden** | 条目级加密/紧急访问/Send/WebSocket | I9 vault-spec-3-security-enhance |
| **prs** | Git 同步 prepare→finalize、Plaintext/Ciphertext 封装 | I9 vault-spec-4-git-sync |
| **ChatGPT** | AppConf::amend 增量合并 | I8 declarative-settings-spec |

---

## 7 新功能检查总表

> 派生索引，不构成判据；与 `docs/principle/` 冲突时以原则为准。

### 7.1 架构

- [ ] IPC 三层：Service / Orchestrator / Store 职责分离？
- [ ] Rust cmd 薄壳（零业务逻辑）？
- [ ] Orchestrator 通过 Port 接口写 store？
- [ ] 多实现模块遵循「核心封闭 / 实现自治 / 编排半封闭」，扩展只改注册表？

### 7.2 类型安全

- [ ] command 返回 DTO struct（非 `serde_json::Value`）？
- [ ] DTO 加 `rename_all = "camelCase"` + `deny_unknown_fields`？
- [ ] 输出 DTO 的 `Option` 未加 `skip_serializing_if`、输入 DTO 的 `Option` 加了 `#[ts(optional = nullable)]`？
- [ ] 前端无 `as` 强转，用泛型 `invoke<T>`？
- [ ] 类型独立文件（`xxx.types.ts`）？
- [ ] 改 DTO 后跑了 `npm run contract:gen` 并提交生成物？

### 7.3 事件

- [ ] event 按三分类选 emit 方式？
- [ ] payload 有 Rust struct（非 `json!({})`）？
- [ ] 数据变更类 `app.emit` 广播？
- [ ] 主窗口 `listen()`（非仅轮询）？

### 7.4 错误

- [ ] `createLogger` 替代裸 `console.error`？
- [ ] 禁止 `.catch(() => {})`？
- [ ] Store 级错误用户可感知？
- [ ] React.lazy 包 ErrorBoundary？
- [ ] 错误有 category + recoveryActions？

### 7.5 安全

- [ ] 敏感操作走 Rust command？
- [ ] 明文类型 Zeroize + Drop？
- [ ] 密钥轮换检查所有派生数据？
- [ ] 锁定→解锁对称？

### 7.6 流式

- [ ] StreamAccumulator + consume_stream 双层？
- [ ] `into_outcome(self)` 消费 self？
- [ ] 流事件携带 `attempt_id`（非 `message_id`）？
- [ ] `attempt_id` 后端生成？
- [ ] `chat-stream-start` 帧协议？
- [ ] 所有事件按 `attempt_id` 过滤？
- [ ] cancel→send 原子切换？

### 7.7 Tool

- [ ] 新 OS 工具 Tool trait + Registry（非独立 command）？
- [ ] ToolBox 直达执行（不经 ToolPipeline）？
- [ ] Agent 经 ToolPipeline 审批？
- [ ] 双入口共享 trait 实现？

### 7.8 风格

- [ ] 命名对齐先例？
- [ ] 目录组织符合 `docs/principle/CODE-SPLIT-PRINCIPLE.md` §4 模式与本文 §2 约定？
- [ ] re-export 保持外部 API 不变？
- [ ] `as unknown as` 仅 2 处？
- [ ] 设置项同步 Rust struct + `*ConfDto` 并跑 `contract:gen`？
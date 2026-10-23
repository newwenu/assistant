# 项目设计指导纲要

> 日期：2026-10-23｜性质：**分析指导**｜来源：I1~I20 全阶段 + I9/I2e/I18 进行中

---

## 1 架构分层

### 1.1 前端 IPC 三层（I13）

```
Service（IPC 协议）→ Orchestrator（业务编排）→ Store（纯状态）
```

| 层 | 职责 | 禁止 |
|---|------|------|
| Service | `invoke` 封装 + 类型转换 | 业务逻辑、store 读取 |
| Orchestrator | 流程编排、流式状态机、事件分发 | `invoke`、直接 set store（通过 Port 回调） |
| Store | state shape + 简单 set/get | `invoke`、业务编排、跨 Store 读取 |

**三零原则**：Orchestrator 零 store 依赖（Port 接口可独立单测）、Service 零业务逻辑、Store 零 invoke。

### 1.2 前端注册表三层（I5-Ext / I14 / I8）

多实现模块（渲染器/策略/设置项）采用 Core → Implementation → Orchestration 三层：

- **Core（封闭）**：跨实现共享类型 + 通用工具 + 注册表
- **Implementation（开放）**：各实现自治，同层不可交叉依赖
- **Orchestration（半封闭）**：调度 + 状态 + 桥接

**注册表对齐**：`registerRenderer(entry)` / `defineSetting(def)` / `defineTool()` 统一模式——能力声明 → 自动适配 → 扩展不改 Core。

**数据驱动原则**（I14）：硬编码映射 → 数据化规则对象 + 求值逻辑；半开放类型用 branded string（`string & { __brand }`）而非固定 union。

### 1.3 Rust cmd 薄壳（I9 / I19）

```
cmd/（参数解包 → 调 crate → 错误映射 → 返回）
  → crate 业务层 → storage 层 → crypto 层
```

cmd 层零业务逻辑；辅助函数被多子模块共享时留 `mod.rs` 作 `pub(crate) fn`。

---

## 2 Store 拆分（I13-Ext / I9）

Store 超 500 行或 5+ 职责域时按 slice pattern 拆分：

**拆分决策**：

| 拆 | 不拆 |
|---|------|
| 字段生命周期独立（流式 vs 持久化） | 拆后跨 slice 读写增加 |
| | 体量极小（<3 字段 + <2 action） |
| | 拆后 envPort 需跨 store 读取 |

**跨域依赖**：Slice A 可读 Slice B（`get().fieldB`，头部注释 `// 跨 slice 读取`），禁止写；触发 action 用 `get().actionB()` 或组装层注入回调。

**命名**：接口 `ModuleDomainSlice`，文件 `moduleDomainSlice.ts`；slice ≥4 放 `slices/` 子目录，<3 平铺 `core/`。

---

## 3 前后端协议（I16 / I20）

### 3.1 DTO 规范

```rust
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct XxxDto {
    pub required: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub optional: Option<String>,
}
```

- `deny_unknown_fields`：增删字段立即报错，强制前后端同步
- `#[serde(default)]`：新字段前向兼容，旧版前端不崩溃
- 禁止 `json!({})` 手动构造 event payload（22 处反面教材）
- 禁止前端 `as` 强转 invoke 返回值（18 处反面教材），改用泛型 `invoke<T>`

### 3.2 Event 三分类（I20）

| 职能 | emit | 判断 |
|------|------|------|
| 数据变更通知 | `app.emit` 广播 | "数据变了"，所有关心该数据的窗口都应收到 |
| 窗口 UI 控制 | `window.emit` 定向 | "某窗口做 UI 动作"，仅目标窗口响应 |
| 高频独占流 | `window.emit` 定向 | 某窗口独占渲染流 |

**关键**：元数据操作（删除/重命名/置顶/复制等 9 个命令）执行后必须 emit `conversation-list-changed`，否则其他窗口永远不知道数据变了。`batch_deleted` 必须携带 `affected_ids`，否则前端无法精确清理选中状态。

### 3.3 类型文件（I9 / I16）

类型与 Service 解耦，独立 `xxx.types.ts` 文件。新模块优先点号分隔（`vault.types.ts` / `vault.service.ts` / `vaultStore.ts` 命名族）。

---

## 4 错误处理（I17 / I19）

**六层防御**：UI 组件 → ErrorBoundary → 全局处理器 → createLogger → 结构化错误 → Rust crash_dump

**硬规则**：
1. 禁止 `.catch(() => {})` 静默吞没（至少 `log.warn`）
2. 禁止裸 `console.error`（统一 `createLogger(context)`)
3. Store 级错误必须用户可感知（通知系统）
4. React.lazy 必须包裹 ErrorBoundary

**错误类别感知恢复**（非仅 `retryable: boolean`）：

| category | 引导动作 | 禁止动作 |
|----------|---------|---------|
| ContextOverflow | "压缩上下文"或"新建对话" | "重试"（必然再溢出） |
| AuthFailed | "打开模型设置" | "重试" |
| RateLimited | "等待 N 秒后重试"（带倒计时） | 无倒计时的"重试" |
| ContentPolicy | "复制诊断" | 提供重试 |

**统一重试原语**：所有重试/重发/编辑重发收敛到 `resendFrom(pivotTimestamp, content)`，禁止多条路径。重试必须精确关联出错消息 ID，错误事件需幂等保护（terminal dedup），后端 sleep 必须检查 cancel_token。

---

## 5 职责拆分执行（I9 / I19）

### 5.1 拆分前审计

| 审计项 | 方法 |
|--------|------|
| 职责域划分 | 识别独立职责域（字段 + action + 依赖） |
| 跨域依赖图 | 画出域间读写，标注拆后增加的跨域读写 |
| 功能重叠 | 检查两文件做相同 invoke（如 historyService vs chatService 6 个重复方法） |
| 静态/动态混杂 | 静态注册表与 Zustand 动态状态混同文件 |
| 纯函数/副作用混杂 | 纯函数与适配器/工厂耦合，无法独立测试 |

### 5.2 拆分后风格对齐（I9 §9 共 16 项）

| 偏差类型 | 判断标准 |
|---------|---------|
| 目录组织 | 模块 >500 行或 command >20 时目录拆分合理 |
| slice 目录 | slice ≥4 放子目录（vault 7 vs chatStore 3） |
| 命名约定 | 对齐先例：`ChatCompactSlice` → `VaultLifecycleSlice` |
| 组装写法 | `(set, get)` 显式参数，不用 `(...a)` |
| 子组件前缀 | 子目录内组件不加父名中缀（`blocks/EditBlock.tsx` 先例） |
| 辅助函数归属 | 多子模块共享 → `mod.rs` 的 `pub(crate) fn` |

### 5.3 外部 API 不变

Rust `pub` 签名不变、前端 re-export 保留、类型从新位置 re-export，消费方零修改。

---

## 6 安全（I9）

**最小 IPC 暴露**：敏感操作走 Rust command，前端不接触明文；明文仅在 Rust 栈短暂存在，用完 zeroize。

**安全类型**：

| 类型 | 约束 |
|------|------|
| `VaultPlaintext` | `secrecy::Secret<Vec<u8>>` + `#[zeroize(drop)]` |
| `MasterKey` | 包装 `[u8; 32]`，不实现 Clone/Debug |
| `CipherAlgorithm` | 枚举，Default = Aes256Gcm，decrypt 首字节识别算法 |

**常见遗漏**：

| 遗漏 | 教训 |
|------|------|
| 改密后未清理 emergency data | 密钥轮换必须检查所有依赖旧密钥的派生数据 |
| `encrypt_field` 硬编码 Aes256Gcm | 设置项已定义 `vaultCipherAlgorithm` 但未读取 |
| `LAST_REVEAL_EXPIRY` 只写不读 | 无消费方的影子副本应删除 |
| SSH 锁定移除 keys 但生物识别解锁未添加 | 锁定→解锁循环必须对称 |

---

## 7 流式架构（I15 / I16 / I17b）

### 7.1 双层分离（I15）

```
StreamAccumulator（累积状态）  — apply_delta() → ApplyDeltaOutcome；into_outcome(self) → StreamOutcome
consume_stream（主循环）       — 纯 async，tokio::select! { cancel / next_item }，自身无业务逻辑
```

- `into_outcome(self)` 消费 self：防半构造状态误用
- `ApplyDeltaOutcome` 结构化返回：调用方精确响应不同事件类型
- `has_effective_output`：仅用户可见输出（text/tool call）设 true，thinking 不算
- `partial_recovery_reason`：零成本预留，扩展时无需改签名

### 7.2 attempt_id 身份标识（I16）

**核心**：`message_id` = 持久化身份，`attempt_id` = 流事件归属，语义不混淆。

| 要素 | 要求 |
|------|------|
| 生成 | 后端（前端禁止造临时 ID） |
| 传递 | 每帧携带 `attempt_id` |
| start 帧 | 流开始前发 `chat-stream-start`（含 `attempt_id`） |
| 过滤 | 所有事件类型统一按 `attempt_id` 过滤 |
| 兼容 | `attempt_id` optional，旧事件退化为不过滤 |

**反面教材**（I16 共 18 个不对齐点，致命 6 个）：
- 前端 `temp-assistant-${Date.now()}` vs 后端 `Uuid::new_v4()` → 过滤永远失效
- `finish_reason` 无过滤 → 旧请求 stale finish 提前结束新流
- cancel 清 `streamingMessageId=null` 再 send → 时序窗口内所有事件被丢弃
- coalescer/seqDedup 用 `message_id` 做 key → seq 空间碰撞误拒合法 delta

### 7.3 对话分支（I17b）

编辑/重试/重新生成从截断式改为分支式。已有 `parent_id` DB 列（未使用），需新增 `sibling_index` + `active_branch_id` + `BranchPicker` UI（`hideWhenSingleBranch`）。分支上限 20/parent，达上限提示 Fork 到新对话。

---

## 8 声明式设置（I8）

**问题**：新增设置项改 8-12 文件。**解决**：声明式注册表 + `patch_conf` 增量合并，降至 1-2 文件。

`AppConf::amend`：`BTreeMap` 合并 patch → 反序列化校验 → `sideEffectRegistry` 遍历执行副作用。

**新增设置项三步**：① Rust struct 加字段 + `#[serde(default)]` ② TS interface 加同名字段 ③ `definitions.ts` 调 `defineSetting()` 注册。禁止 `Record<string, unknown>` 传递部分字段。

---

## 9 Provider 多 API（I9-Provider）

```
base.rs（trait + 错误类型）→ client/（HTTP + SSE + quirks）→ types/（UnifiedResponse）→ {openai,anthropic,gemini}/（各 6 文件）→ stream/
```

| 原则 | 说明 |
|------|------|
| UnifiedResponse | 各 adapter 响应统一为统一类型，上层不感知 Provider 差异 |
| quirks 隔离 | URL 检测/reasoning 兼容/header 差异集中 `quirks.rs` |
| SSE 解析 | 用 eventsource_stream crate，禁止手动 `buffer.find('\n')` |
| 流内错误 | 部分 provider 在流中返回错误，必须捕获 |

---

## 10 Tool 双入口（I18 / I18b）

```
Tool trait 实现（共享）→ ToolRegistry（共享注册）
  ├── ToolBox：os_tool_exec → find → execute() → 返回（直达，不经 ToolPipeline）
  └── Agent：RoundExecutor → ToolPipeline → ToolGateway（审批）→ execute() → ResultStorage（审计）
```

ToolBox 不经 ToolPipeline：用户主动点击 = 权限主体，审批自己无意义，审计追踪 AI 行为而非用户操作。

**统一桥接**：Rust 侧永远只有一个 `os_tool_exec` command。新增工具三步：① Tool trait + Registry 注册 ② `os.ts` 加命名空间 ③ React 组件 + `defineTool()`。**禁止**为单工具新增独立 Tauri command。

---

## 11 常见陷阱速查

### 类型安全

| 陷阱 | 教训 |
|------|------|
| `as` 强转 invoke 返回值 | Rust 改签名前端不报错，运行时才崩 → 改泛型 `invoke<T>` |
| `json!({})` 构造 payload | 遗漏字段无感知 → 用 struct + `deny_unknown_fields` |
| `tool_calls_json` 双重语义 | 同字段前后端不同类型解释 → DTO 层统一 |
| `provider.models` 散落解析 | 9 处 `JSON.parse` 无 try-catch → Service 层集中解析 |

### 状态管理

| 陷阱 | 教训 |
|------|------|
| 事件绕过 Store | `store.setState()` 直接调用 → 状态变更路径不透明 |
| cancel→send 时序窗口 | `streamingMessageId=null` 期间所有事件被丢弃 → attempt_id 原子切换 |
| compact 遗漏字段重置 | 未重置 `isReasoningStreaming` → mascot 卡死"思考中" → 重置必须完整 |

### 安全

| 陷阱 | 教训 |
|------|------|
| `GetForegroundWindow` 跨进程 | `WTSRegisterSessionNotification` 要求本进程 hwnd → 屏幕锁检测失效 |
| ClipboardGuard 持锁做 IO | 恢复原内容时持锁写剪贴板 → 死锁 |
| HIBP 串行请求 | 500×200ms=100s 无反馈 → `buffer_unordered` + Semaphore 限速 |

### 设置系统

| 陷阱 | 教训 |
|------|------|
| 新增设置项改 8-12 文件 | 声明式注册表 + amend 降至 1-2 文件 |
| 副作用硬编码 `if prev != conf` | `sideEffectRegistry` 注册，不封闭 |
| 全量 set 命令 | `patch_conf` 增量合并，减少传输 + 避免覆盖 |

---

## 12 风格约定速查

### Rust

| 约定 | 说明 |
|------|------|
| `pub mod` 平铺 | `lib.rs` 声明子模块，子模块按需建目录 |
| 跨文件 `impl` | 允许，但 trait impl 必须在定义 struct 或定义 trait 的文件 |
| `pub use xxx::*` | 子模块公开项提升到父命名空间 |
| 共享辅助留 `mod.rs` | 可见性 `fn`（crate-private 即够） |
| 测试放 `tests/` | 集成测试不在 `src/` 内 |
| `#[cfg(target_os)]` 集中 | 平台代码独立文件，不污染主文件 |
| cmd <500 行单文件 | >500 行或 >20 command 用目录 `cmd/xxx/` |

### 前端

| 约定 | 说明 |
|------|------|
| Store <500 行 | 超过按 slice pattern 拆分 |
| slice `(set, get)` 显式参数 | 不用 `(...a)` 解构 |
| slice 接口 `ModuleDomainSlice` | 对齐 `ChatCompactSlice` 先例 |
| 子组件不加父名中缀 | `unlock/PasswordForm.tsx` 非 `PasswordUnlockForm.tsx` |
| CSS Module 共享 | 同模块子组件共享同一 CSS 文件 |
| 测试与源同目录 | `vaultStore.test.ts` 放 `vault/core/` |
| `as unknown as` 仅 2 处 | `confUtils.ts` + `i18n/index.ts`，其余禁止 |
| 新增设置项同步接口 | `AppConf`/`OrbConf`/`MascotConf` + Rust struct + `#[serde(default)]` |

---

## 13 交付策略（I9 / I2e）

**阶段原则**：每阶段独立可测、不破坏现有行为、依赖显式（前置 ✅ 后才开始）、已砍阶段显式标注。

**步骤排序**：Rust 核心拆分（编译收益）→ Store 拆分（渲染性能）→ UI 拆分（可维护性）。

**修复分层**：

| 批次 | 风险 | 典型操作 |
|------|------|------|
| 1 | 零 | 删除死代码、只写不读变量 |
| 2 | 低 | 委托替代重复 invoke、类型包装 |
| 3 | 中 | 补全 IPC 三层、补全前端页面 |
| 4 | 较高 | 改用新类型、跨 crate 重构 |

---

## 14 新功能检查总表

### 14.1 架构

- [ ] IPC 三层：Service / Orchestrator / Store 职责分离？
- [ ] Rust cmd 薄壳（零业务逻辑）？
- [ ] Orchestrator 通过 Port 接口写 store？
- [ ] 多实现模块：Core → Implementation → Orchestration + 注册表？
- [ ] 硬编码映射数据化？

### 14.2 类型安全

- [ ] command 返回 DTO struct（非 `serde_json::Value`）？
- [ ] DTO 加 `rename_all = "camelCase"` + `deny_unknown_fields`？
- [ ] Option 字段加 `skip_serializing_if` + `#[serde(default)]`？
- [ ] 前端无 `as` 强转，用泛型 `invoke<T>`？
- [ ] 类型独立文件（`xxx.types.ts`）？

### 14.3 事件

- [ ] event 按三分类选 emit 方式？
- [ ] payload 有 Rust struct（非 `json!({})`）？
- [ ] 数据变更类 `app.emit` 广播？
- [ ] 主窗口 `listen()`（非仅轮询）？

### 14.4 错误

- [ ] `createLogger` 替代<code>console.error</code>？
- [ ] 禁止 `.catch(() => {})`？
- [ ] Store 级错误用户可感知？
- [ ] React.lazy 包 ErrorBoundary？
- [ ] 错误有 category + recoveryActions？

### 14.5 安全

- [ ] 敏感操作走 Rust command？
- [ ] 明文类型 Zeroize + Drop？
- [ ] 密钥轮换检查所有派生数据？
- [ ] 锁定→解锁对称？

### 14.6 流式

- [ ] StreamAccumulator + consume_stream 双层？
- [ ] `into_outcome(self)` 消费 self？
- [ ] 流事件携带 `attempt_id`（非 `message_id`）？
- [ ] `attempt_id` 后端生成？
- [ ] `chat-stream-start` 帧协议？
- [ ] 所有事件按 `attempt_id` 过滤？
- [ ] cancel→send 原子切换？

### 14.7 Tool

- [ ] 新 OS 工具 Tool trait + Registry（非独立 command）？
- [ ] ToolBox 直达执行（不经 ToolPipeline）？
- [ ] Agent 经 ToolPipeline 审批？
- [ ] 双入口共享 trait 实现？

### 14.8 风格

- [ ] 命名对齐先例？
- [ ] 目录对齐（cmd 目录 vs 单文件、slices 子目录 vs 平铺）？
- [ ] re-export 保持外部 API 不变？
- [ ] `as unknown as` 仅 2 处？
- [ ] 设置项同步 interface + Rust struct + `defineSetting()`？

### 14.9 交付

- [ ] 分阶段（每阶段独立可测）？
- [ ] 修复按风险分层？
- [ ] 已砍阶段显式标注？
- [ ] 拆分保持外部 API 不变？

---

## 15 参考项目速查

| 项目 | 核心借鉴 | 借鉴文档 |
|------|---------|---------|
| **OpenBitFun** | Port 接口、Permission Engine、createLogger、lazyWithRecovery、DTO 集中、StreamContext+StreamProcessor、round_executor(attempt_id)、app.emit | I13/15/16/17/18/19 |
| **OpenHuman** | AttachmentStash、ToolPolicy trait、BranchPickerPrimitive、app.emit | I2e/17b/18/20 |
| **deepseek-harness-orb** | LlmAttemptId branded type、start/chunk/end 帧、settle/abandon 双路径 | I15/16 |
| **AIRI** | ApiError 工厂、image-projection 降级、transient retry | I19/2e |
| **KeePassXC** | 屏幕锁/生物识别/TOTP/HIBP/SSH Agent | I9 |
| **Vaultwarden** | 条目级加密/紧急访问/Send/WebSocket | I9 |
| **prs** | Git 同步 prepare→finalize、Plaintext/Ciphertext 封装 | I9 |
| **ChatGPT** | AppConf::amend 增量合并 | I8 |

---

## 16 文档组织

| 目录 | 用途 | 性质 |
|------|------|------|
| `docs/achievement/` | I1-I12 已完成 | spec + tasklist |
| `docs/achievement1/` | I13-I19 部分已完成 | spec + tasklist |
| `docs/store/` | 进行中/待实施 | spec + tasklist + issues + deadcode + responsibility-split |
| `docs/analysis/` | **分析指导** | 跨阶段综合，非执行计划 |
| `docs/designed/` | 顶层设计 | 架构级 |

**新功能文档顺序**：① `analysis/` 可行性分析 → ② `store/` spec + tasklist → ③ `store/` issues + deadcode + responsibility-split
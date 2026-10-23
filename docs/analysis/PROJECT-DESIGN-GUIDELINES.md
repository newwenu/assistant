# 项目设计指导纲要

> 生成日期：2026-10-23
> 性质：**分析指导文档**，综合历史各阶段的设计决策、经验教训与风格约定，为新功能引入提供系统性参考
> 来源文档：I1~I20 全部已完成阶段 + I9/I2e/I18 进行中阶段的 spec/tasklist/issues/deadcode/responsibility-split 文档

---

## 1 架构分层模式

### 1.1 前端三层架构（Service → Orchestrator → Store）

**来源**：I13 Chat 服务层分离

所有涉及 IPC 调用 + 业务编排 + 状态管理的模块，应遵循三层职责划分：

```
┌─────────────────────────────────────────────────────┐
│  Layer 3: Store  — 纯状态容器                        │
│  职责：state shape + 简单 set/get action             │
│  不含：invoke、业务编排、跨 Store 读取                │
├─────────────────────────────────────────────────────┤
│  Layer 2: Orchestrator  — 生命周期编排               │
│  职责：业务流程编排、流式状态机、事件处理分发          │
│  不含：invoke、直接 set store（通过 Port 回调通知）   │
├─────────────────────────────────────────────────────┤
│  Layer 1: Service  — IPC 协议层                      │
│  职责：类型安全的 invoke 封装 + 返回值转换             │
│  不含：业务逻辑、store 读取                           │
└─────────────────────────────────────────────────────┘
```

**关键原则**：
- **Orchestrator 零 store 依赖**：通过 Port 接口（`ChatStatePort` / `ChatEnvPort`）与 store 通信，可独立单元测试
- **Service 零业务逻辑**：仅做 `invoke` + 类型转换，不做任何业务判断
- **Store 零 invoke**：所有 IPC 调用委托 Service，Store 不直接 `invoke`

**新功能检查清单**：
- [ ] 新增 IPC 调用是否集中在 Service 层？
- [ ] 业务编排是否在 Orchestrator 而非 Store？
- [ ] Store 是否仅含状态 + 简单 set/get？
- [ ] Orchestrator 是否通过 Port 接口写 store（而非直接 import store）？

### 1.2 前端多层架构（Core → Renderer → Orchestration）

**来源**：I5-Ext Mascot 可扩展渲染架构、I14 Mascot 表情映射可扩展化

当模块需要支持多种实现（渲染器/引擎/策略）时，采用注册表 + 三层分离：

```
Layer 0: Core（封闭）— 跨实现共享的类型 + 通用工具 + 注册表
Layer 1: Implementations（开放）— 各实现自治（renderers/rive/、renderers/svg/）
Layer 2: Orchestration（半封闭）— 调度 + 状态 + 桥接
```

**依赖规则**：
- 上层可依赖下层，下层不可依赖上层
- 同层不可交叉依赖（`renderers/rive/` 不可 import `renderers/svg/`）
- Core 不含实现特定概念

**注册表模式**（I5-Ext + I8 对齐）：

| 注册入口 | 能力声明 | 自动适配 | 扩展方式 |
|---------|---------|---------|---------|
| `registerRenderer(entry)` | `RendererCapabilities` | UI 根据 capabilities 隐藏/显示 | 新渲染器不改 Core |
| `defineSetting(def)` | `SettingDefinition.type` | UI 根据 type 自动选择控件 | 新设置项不改框架层 |

**数据驱动 + 声明式原则**（I14）：
- 硬编码映射（if-else 链、正则规则、固定 union）→ 数据化规则对象 + 求值逻辑
- 新增行为 = 新增规则数据，不改编排层核心逻辑
- 半开放类型：branded string（`string & { __brand }`）而非固定 union，渲染器声明 `supportedFaces`，不支持的 fallback

### 1.3 Rust 侧 cmd → crate 内部分层

**来源**：I9 Vault spec、I19 代码职能检查

```
cmd/ 层（Tauri command 薄壳）
  → crate 内部业务层（SecurityDbState / SyncEngine / ChatRuntime）
    → storage 层（rusqlite CRUD）
      → crypto 层（加解密原语）
```

**关键原则**：
- **cmd 层不做业务逻辑**：仅做参数解包 → 调用 crate 方法 → 错误映射 → 返回
- **业务逻辑在 crate 内部**：cmd 层的 `vault_init` 只是 `state.init(path).map_err(|e| e.to_string())`
- **辅助函数留在 `mod.rs`**：被多个子模块共享的工具函数（如 `vault_state`、`cipher_algo`）放在 `cmd/vault/mod.rs`，子模块通过 `super::` 引用

---

## 2 Store 拆分模式

### 2.1 Zustand Slice Pattern

**来源**：I13-Ext Chat Store 细粒度拆分、I9 Vault 职责拆分

当 Store 超过 500 行或含 5+ 职责域时，按 slice pattern 拆分：

```typescript
// slice 工厂函数签名
export function createXxxSlice(
  set: (partial: Partial<FullState> | ((state: FullState) => Partial<FullState>)) => void,
  get: () => FullState,
): XxxSlice { ... }

// 组装
export const useStore = create<FullState>()((set, get): FullState => {
  const sliceA = createSliceA(set, get);
  const sliceB = createSliceB(set, get);
  return { ...sliceA, ...sliceB };
});
```

**拆分决策标准**（来自 I13-Ext 的经验）：

| 条件 | 拆 | 不拆 |
|------|-----|------|
| 字段生命周期独立（如流式标量 vs 持久化） | ✅ | |
| 拆分后跨 slice 读写反而增加 | | ✅ |
| 体量极小（<3 字段 + <2 action） | | ✅ |
| 拆分后 envPort 需跨 store 读取 | | ✅ |

**命名约定**：
- 接口：`VaultLifecycleSlice`（模块前缀 + 域名 + `Slice` 后缀）
- 文件：`vaultLifecycleSlice.ts`
- 目录：3 个以下 slice 平铺在 `core/`；4 个以上放 `slices/` 子目录

### 2.2 Slice 间的跨域依赖处理

**来源**：I13-Ext 跨职责域依赖分析

| 依赖类型 | 处理方式 |
|----------|---------|
| Slice A 写 Slice B 的字段 | ❌ 禁止。通过 `set({ ... })` 只修改自己域的字段 |
| Slice A 读 Slice B 的字段 | ✅ 允许。通过 `get().fieldB` 读取（FullState 包含所有字段） |
|&nbsp;&nbsp;但需标注为"跨 slice 读取" | 在 slice 文件头部注释 `// 跨 slice 读取: get().isLocked` |
| Slice A 需触发 Slice B 的 action | 通过 `get8get().actionB()` 调用，或在组装层注入回调 |

---

## 3 前后端协议设计

### 3.1 类型安全契约

**来源**：I16 前;端协议4;端消息协议统一、I16 命名迁移

**分层架构**：

```
后端：
  storage/ 层 — snake_case（磁盘格式不变）
A
  protocol/dto.rs — #[serde(rename_all = "camelCase")] + deny_unknown_fields
  cmd/ 层 — 返回 DTO，emit DTO

前端：
  Tauri 边界 — camelCase（后端 DTO 序列化输出）
  业务层 (Store/Component/Hook) — camelCase
```

**DTO 注解规范**（新功能必须遵守）：

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct XxxDto {
    pub required_field: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub optional_field: Option<String>,
}
```

- `deny_unknown_fields`：前后端增删字段时立即报错，强制同步
- `skip_serializing_if`：高频 event payload 省略 None 字段，减少传输体积
- `#[serde(default)]`：新增字段加此注解，旧版前端不崩溃（前向兼容）

### 3.2 Event 设计三分类

**来源**：I20 跨窗口事件同步

| 职能 | emit 方式 | 判断依据 |
|------|----------|---------|
| **① 数据变更通知** | `app.emit` 广播 | "数据发生了什么变化"，关心该数据的窗口都应收到 |
| **② 窗口 UI 控制** | `window.emit` 定向 | "某个窗口应该做什么 UI 动作"，只有目标窗口响应 |
| **③ 高频独占流** | `window.emit` 定向 | 某窗口独占的渲染流，其他窗口不需要逐帧接收 |

**新功能检查清单**：
- [ ] 新增 event 是否按上述三分类选择了 emit 方式？
- [ ] 数据变更类 event 是否用 `app.emit` 广4;非 `window.emit`？
- [ ] event payload 是否有对应 Rust struct（禁止 `json!({})` 手动构造）？
- [ ] event payload struct 是否加了 `deny_unknown_fields`？

### 3.3 前端类型文件组织

**来源**：I9 Vault 职责拆分 §9.8、I16 命名迁移

| 模块 | 类型文件位置 | �'名约定 |
|------|-------------|---------|
| chat | `src/chat/core/chatTypes.ts` | `chatTypes.ts`（驼峰） |
| vault | `src/vault/core/vault.types.ts` | `vault.types.ts`（点号，与 `vault.service.ts` 一致） |
| settings | `src/settings/core/types/` | 独立 `types/` 子目录 |

**原则**：类型与 Service 解耦。类型定义独立文件，Service `import type` 引用。新模块优先采用 `xxx.types.ts`（点号分隔）命名，与 `xxx.service.ts` / `xxxStore.ts` 构成命名族。

---

## 4 错误处理体系

### 4.1 分层防御

**来源**：I19 统一错误处理体系

```
Layer 6: UI 组件 — ErrorBlock / RetryBanner / GlobalErrorToast
Layer 5: ErrorBoundary — App级 → Feature级 → 懒加载级
Layer 4: 全局错误处理器 — window.error + unhandledrejection
Layer 3: 统一日志系统 — createLogger + Tauri 持久化 + console 劫持
Layer 2: 结构化错误定义 — ChatError / AppError (含 category + recovery)
Layer 1: Rust crash_dump — panic hook + crash_handler
```

**新功能必须遵守**：
1. **禁止 `.catch(() => {})` 静默吞没**：至少 `log.warn`，除非是尽力而为的操作（如 generation 递增）
2. **禁止裸 `console.error`**：统一用 `createLogger(context).error()`
3. **Store 级错误必须用户可感知**：通过通知系统推送，不能仅 console
4. **React.lazy 必须包裹 ErrorBoundary**：代码分割加载失败不能冒泡至白屏

### 4.2 错误类别感知恢复

**来源**：I17 Chat 重发/重试A;试恢复

```typescript
// 错误不应仅有 retryable: boolean
interface StructuredError {
  category: ErrorCategory;      // network / auth / rate_limit / context_overflow / ...
  severity: 'warning' | 'error';
  retry#coveryActions: RecoveryAction[];  // [{code;code: 'retry'}, {code: 'open:open_settings'}]
  diagnostics: string;          // 可复制的诊断信息
}
```

**恢复动作映射原则**（来自 I17 对 OpenBitFun 的参考）：
- `ContextOverflow` → 引导"压缩上下文"或"新建对话"，而非"重试"（重试必然再次溢出）
- `AuthFailed` → 引导"打开模型设置"，而非"重!重试"
- `RateLimited&` → 引导"等待 N 秒后重试"（带倒计时），而非无倒计时的"重试"
- `ContentPolicy` → 不提供重试（`retryable: false`），仅"复制诊断"

### 4.3 统一重试原语

**来源**：I17

所有重试/C;重发/编辑重=9;发操作收敛到共享原语 `resendFrom(pivotTimestamp, content)`，禁止多条行为不一致的路径。

**关键约束**：
- 重试操作必须**精确关联到出错的 assistant 消息 ID**，禁止"找最后一条 user 消息"的模糊定位$位,定位
4;式- 错误事件处理需**幂等保护**（terminal dedup），防止重复事件导致重复状态变更
- 后端重试循环的 `sleep` 必须检查 `"cancel_token`，用户可中断等待中的重试

---

## 5 职责拆分执行规范

### 5.1 拆分前必做：代码审计

**来源**：I19 代码职能检查、I9 Vault 职责拆分

拆分前必须完成以下审计：

| 审计项 | 方法 |
|--------|------|
| 职责域划分 | 识别文件内的独立职责域（状态字段 + action + 依赖关系） |
| 跨域依赖图 | 画出域间读写依赖，标注哪些拆分后跨域读写会增加 |
| 功能D'功能重叠 | 检查是否有两个文件"文件做相同 invoke（如 historyService vs chatService 的 6 个重复方法） |
| �E;态/动态混杂 | 检查是否静态注册表与 Zustand 动态状态混在同一文件 |
| 纯函数/副作用混杂 | 检查纯函数是否与适配器/工厂函数耦合，导致无法独立测试 |

### 5.2 拆分后必做：风格对齐审查

**来源**：I9 Vault 职责拆分 §9（16 项审查）

拆分方案必须逐项对比项目现有约定，检查是否对齐。已发现的偏差类型：

| "偏差类型 | 来源 | 判断标准 |
|-----------|------,------|---------|
| 目录组织偏差 | `cmd/vault/` 是3;cmd 层首个目录 | 模块行数 >500 或 command 数 >20 时目录拆分合理 |
| slice 目录偏差 | `slices/` 子目录 vs 平铺 | slice 数 ≥4 时子目录更清晰（vault 7 个 vs chatStore 3 个） |
| 命名约定偏差 | `LifecycleState` vs `VaultLifecycleSlice` | 对齐已有先例（chatStore 用 `ChatCompactSlice`） |
| 组装写法偏差 | `(...a)`B`set, get)` | 对齐已有先例（chatStore 用显式参数） |
| UI 子组件前缀偏差 | `PasswordUnlockForm` vs `PasswordForm` | 子目录<组件不加父组件名中缀（`blocks/EditBlock.tsx` 先例） |
| 辅助函数归属偏差-| `custom_fields_eq` 放 `import_export.rs` vs `mod.rs` | 被多个子模块共享时留 `mod.rs8` 作 `pub(crate) fn` |

### 5.3 外部 API 不变原则

**来源**：I9 Vault 职责拆分 §6.3、I13 Chat 服务层分离

拆分必须保持外部 API 不变：

- **Rust**：`pub` 方法签名不变，仅文件组织变化；`invoke_handler` 注册路径不变
- **前端**：`export { useVaultStore }` re-export 保留，所有外部 import 路径不变
- **类型**：`export type` 从新位置 re-export，消费方无需修改

---

## 6 安全设计原则

### 6.1 最小 IPC �&露

**来源**：I9 Vault spec

- 敏感操作（解密、揭示）走 Rust command，前端不接触明文
- >明文仅在 Rust 栈上短暂存在，用完立即 zeroize（`VaultPlaintext` / `MasterKey` 实现 `Zeroize + Drop`）
- 前端仅通过 `RevealResult { value, expires_at }` 获取临时明文，30 秒后自动清零

### 6.2 安全类型系统

**来源**：I9 Vault spec §3.3

| 类型 | 约束 |
|------|------|
| `VaultPlaintext` | `secrecy::Secret<Vec<u8>>`，访问必须通过 `expose()`；`#[zeroize(drop)]` |
| `MasterKey` | 包装 `[u8; 32]`，不实现 Clone/Debug（防止意外复制/打印） |
| `PerEntryKey` | 条目删除后不可复用（无持久化路径） |
| `CipherAlgorithm` | 枚举，Default = Aes256Gcm，decrypt 首字节识别算法（前向兼容） |

### 6.3 安全相关常见遗漏

**来源**：I9 Vault issues

| 遗漏类型 | 具体案例 | 教训 |
|----------|---------|------|
| 改密后未清理关联数据 | `change_master_password` 未删除 emergency data | 密钥轮换必须检查所有依赖旧密钥的派生数据 |
| 硬编码算法 | 所有 `encrypt_field` 调用硬编码 `Aes256Gcm` | 设置项已定义 `vaultCipherAlgorithm` 但未读取，双算法支持形同虚设 |
| 只写不读的原子变量 | `LAST_REVEAL_EXPIRY` 只 `.store()` 无 `.load()` | 影子副本增加认知负担，如无消费方应删除 |
| SSH 锁定/解锁不对称 | 锁定时移除 SSH keys，但生物识别解锁时未自动添加 | 安全操作的"锁定→解锁"循环必须对称检查 |

---

## 7 跨窗口与事件设计

### 7.1 主窗口零监听问题

**来源**：I20

**反面教材**：主窗口 `HistoryManager` 仅靠 15s 轮询，Orb 的变更最多延迟 15 秒才能在主窗口体现。

**正确做法**：后端所有数据变更都 `app.emit` 广播，主窗口注册 `listen()` 实时响应。

### 7.2 元数据操作必须发事件

**来源**：I20 §1.2

9 个修改对话列表结构的后端命令（删除/重命名/置顶/复制/切换模型/批量删除/清空/新建），执行后必须 emit `conversation-list-changed` 事件。**不发事件 = 其他窗口永远不知道数据变了**。

### 7.3 `batch_deleted` 必须携带 `affected_ids`

**来源**：I20 §4.2

如果只传 `None`，前端无法判断当前选中的对话是否#否在被批量删除的集合中——要么保守地无条件清空选中状态（误伤），要么不做清理（遗漏）。携带 ID 列表可精确匹配。

---

## 8 项目风格约定速查

### 8.1 Rust 约定

| 约定 | 说明 | 来源 |
|------|------|------|
| crate 内 `pub mod` 平铺 | `lib.rs` 用 `pub mod` 声明子模块，子模块自身按需建目录 | I9 §9.1 |
| 跨文件 `impl` 同一 struct | Rust 允许同一 crate 内跨文件 `impl`，但 trait impl 必须在定义 struct 或定义 trait 的文件中 | I9 §6.1 |
| `pub use xxx::*` re-export | 子模块的公开项通过 `pub use` 提升到父命名空间 | I9 §9.3 |
| `with_conn_and_key` 留 `mod.rs` | 子模块共享的辅助方法留在主文件，可见性 `fn`（crate-private 足够） | I9 §9.13 |
| 测试放 `tests/` 目录 | 集成测试在 `tests/`，不在 `src/` 内 | I9 §9.11 |
| 条件编译 `#[cfg(target_os)]` 集中 | 平台相关代码集中在独立文件（如 `biometric_ops.rs`），不污染主文件 | I9 §3.1 |

### 8.2 前端约定

| 约定 | 说明 | 来源 |
|------|------|------|
| Store 文件 <500 行 | 超过则按 slice pattern 拆分 | I13-Ext |
| slice 工厂 `(set, get)` 显式参数 | 不用 `(...a)` 解构 | I9 §9.6 |
| slice 接口 `ModuleDomainSlice` 命名 | 对齐 `ChatCompactSlice` 先例 | I9 §9.7 |
| 子组件不加父组件名中缀 | `unlock2unlock/PasswordForm.tsx` 非 `PasswordUnlockForm.tsx` | I9 §9.9 |
| CSS Module 共享 | 同模块子组件共享同一 CSS 文件，不创建新 CSS Module | I9 §9.10 |
| 测试文件与源文件同目录 | `vaultStore.test.ts` 放 `vault/core/` | I9 §9.12 |
| `as unknown as` 仅允许 2 处 | `confUtils.ts` + `i18n/index.ts`，其余禁止 | 项目规则 |
| 新增设置项必须?同步更新接口 | `AppConf`/`OrbConf`/`MascotConf` interface + Rust struct 同名字段 + `#[serde(default)]` | 项目规则 |

### 8.3 目录组织约定

| 场景 | 约定 | 来源 |
|------|------|------|
| Rust cmd 模块 <500 行 | 单文件 `cmd/xxx.rs` | I9 §9.2 |
| Rust cmd 模块 >500 行 | 目录 `cmd/xxx/` + `mod.rs` + 子文件 | I9 §9.2 |
| Rust crate 子模块 | "每功能域一文件"或"每域一目录"，按体量决定 | I9 §9.1 |
| 前端 slice*模块 `core/` | 组装 + 类型 + 服务 + hook + 测试 + `slices/` 子目录 | I9 §4.1 |
| 前端 UI 子目录 | `unlock/`、`forms/`、`detail/`、`settings/` | I9 §9.9 |

---

## 9 分阶段交付策略

### 9.1 阶段划分原则

**来源**：I9 Vault spec §2.1、I2e 多模态附件 spec

| 原则 | 说明 |
|------|------|
| **每阶段独立可测** | 不依赖后续阶段的代码即可编译+测试通过 |
| **不破坏现有行为** | 渐进式迁移，每个 Step 是纯重构或纯新增，不混合 |
| **依赖关系显式** | P1-A → P1-B → P1-C，前置阶段标记 ✅ 后才开始后续 |
| **已砍掉阶段显式标注** | `~~P2B WebDAV~~（Git 同步已够用）`，避免未来重复讨论 |

### 9.2 阶段内步骤+步骤排序

**来源**：I9 Vault 职责拆分 §5

1. **Rust 侧核心拆分**（编译收益最高，改 CRUD 不重编译 biometric）
2. **前端 Store 拆分**（渲染性能收益，组件可按域订阅）
3. **UI 组件拆分**（可维护性收益，容器+子组件+hook）

### 9.3 修复优先级分层

**来源**：I9 Vault deadcode

| 批次 | 风险 | 典型操作 |
|------|------|---------|
| 第一批：可直接删除 | 零 | 删除死代码、删除只写不读的变量 |
| 第二批：重构消除重复 | 低 | 委托替代重复 invoke、类型包装（SafePassword） |
| 第三批：补全规格未完成项 | 中 | 补全 IPC 三层、补全前端页面、集成已有实现 |
| 第四批：重构架构 | 较高 | 改用新类型（VaultEntryDecrypted）、跨 crate 重构 |

---

## 10 常见陷阱与反面教材

### 10.1 类型安全陷阱

| 陷阱 | 案例 | 教训 |
|------|------|------|
| `as` 强转无编译时校验 | 18 处 `invoke` 返回 `serde_json::Value`，前端 `as` 强转 | Rust 端改签名，前端不报错，运行时才崩 |
| `json!({})` 手动构造 event payload | 22 处，遗漏字段无编译时感知 | 必须用 struct +6; `deny_unknown_fields` |
| `tool_calls_json` 双重语义 | `Option<String>` (JSON string) vs `ToolCall[]` (parsed array) | 同一字段在前后端有不同类型解释，需 DTO 层统一 |
| `provider.models` 散落解析 | 9 处 `JSON.parse(provider.models) as ModelConfig[]` 无 try-catch | 集中E/Service 层统一解析+5;错误处理 |

### 10C.2 状态管理陷阱

| 陷阱 | 案例 | 教训 |
|------|------|------|
| 事件绕过 Store | `chatDeltaEvents.ts` 直接 `store.setState()` | 状态变更路径不透明，调试困难 |
| `editMessage` 前后端 timestamp 不一致 | 后端改 timestamp 为 `now()`，前端不改 | 截断逻辑依赖 timestamp，不一致导致截断位置错误 |
| cancel→send 时序窗口 | cancel 完成后 isStreaming=false 但新 send 还没开始 | 延迟的 error 事件可能被当前 attemptId 处理 |
| `compact` 错误处理遗漏 `isReasoningStreaming: false` | compact 失败时 mascot 表情卡死在"思考中" | 重置流式状态必须完整7;量重置所有相关字段 |

### 10.3 安全陷阱

| 陷阱 | 案例 | 教训 |
|------|------|------|
| `GetForegroundWindow` 获取其他进程窗口 | `WTSRegisterSessionNotification` 要求 hwnd 属于当前进程 | 屏幕锁事件完全无法检测，自动锁定功能失效 |
| ClipboardGuard 持锁做 IO | 恢复原内容时持锁写剪贴板 | 死锁风险 |
| HIBP 串行请求 | 500 条目 × 200ms = 100 秒，无进度反馈 | 用 `buffer_unordered` + Semaphore 限速 |

### 10.4 流式架构陷阱

| 陷阱 | 案例 | 教训 |
|------|------|------|
| 前端造临时 ID 与后端 UUID 不匹配 | `temp-assistant-${Date.now()}` vs `Uuid::new_v4()` | 流事件过滤永远失效，所有 error 被静默丢弃 |
| `message_id` 兼做流归属标识 | `message_id` 语义是持久化身份，不是流事件归属 | 引入独立 `attempt_id`，两者语义不混淆 |
| cancel→send 时序窗口 | cancel 清 `streamingMessageId=null` 后再 send | 窗口内所有事件（含新请求的）被丢弃 |
| `finish_reason` 无过滤 | 旧请求的 stale finish 触发 `finishStream()` | 提前结束新流 |
| coalescer 用 `message_id` 做 key | 旧请求和新请求的 seq 空间碰撞 | 误拒新请求的合法 delta |
| `editMessage` 前后端 timestamp 不一致 | 后端改 timestamp 为 `now()`，前端不改 | 截断逻辑依赖 timestamp，不一致导致截断位置错误 |
| compact 错误处理遗漏字段重置 | compact 失败时未重置 `isReasoningStreaming: false` | mascot 表情卡死在"思考中" |

### 10.5 设置系统陷阱

| 陷阱 | 案例 | 教训 |
|------|------|------|
| 新增设置项改 8-12 文件 | Rust struct → cmd → register → TS interface → event → listen → dispatch → UI | 声明式注册表 + amend 增量合并，降至 1-2 文件 |
| 副作用硬编码在 `if prev.xxx != conf.xxx` | 新增副作用需改 Rust 代码 | 副作用通过 `sideEffectRegistry` 注册，不封闭 |
| `Record<string, unknown>` 传递部分字段 | 绕过类型检查 | 禁止；必须同步更新 interface + Rust struct |
| 全量 set 命令 | `set_app_conf` 传整个 config 对象 | 改为 `patch_conf` 增量合并，减少传输 + 避免覆盖 |

---

## 11 流式架构与身份标识

### 11.1 流式消费双层分离

**来源**：I15 Chat Runtime 提取

流式消费拆为两层，对齐 OpenBitFun `StreamContext` + `StreamProcessor` 模式：

```
StreamAccumulator（累积状态层）
  — 独立 struct，持有所有中间累积状态
  — apply_delta() → 返回结构化信号（ApplyDeltaOutcome）
  — into_outcome(self) → 消费 self，构造 StreamOutcome（消费语义防半构造误用）

consume_stream（主循环层）
  — 纯 async 函数，只做"循环 + 取消 + 超时 + 委托"
  — 自身无业务逻辑
  — tokio::select! { cancel / next_item }，TimedStreamItem 枚举统一处理流结束和超时
```

**关键设计决策**：
- **`into_outcome(self)` 消费 self**：构造 outcome 后原 accumulator 不可再用，避免半构造状态误用
- **`ApplyDeltaOutcome` 结构化返回**：`apply_delta` 不返回 void，返回结构化信号让调用方精确响应
- **`has_effective_output` 标记**：仅收到用户可见输出时设 true，thinking/reasoning 不算，用于 partial recovery 决策
- **`partial_recovery_reason` 零成本预留**：I15 正常完成时为 None，I16 扩展时无需改签名

### 11.2 流尝试身份标识（attempt_id）

**来源**：I16 流尝试身份对齐

**核心原则**：`message_id` 只做消息持久化身份，`attempt_id` 只做流事件归属标识，两者语义不混淆。

| 设计要素 | 要求 |
|---------|------|
| 标识生成方 | 后端（前端不造临时 ID） |
| 标识传递方式 | 每帧/每事件携带 `attempt_id` |
| start 帧协议 | 后端在流开始前先发 `chat-stream-start` 事件（含 `attempt_id`） |
| 前端过滤 | 所有事件类型统一按 `attempt_id` 过滤 |
| 过渡兼容 | `attempt_id` 为 optional，旧事件无该字段时退化为不过滤 |

**反面教材**（I16 发现的 18 个不对齐点）：
- 前端生成 `temp-assistant-${Date.now()}` 与后端 `Uuid::new_v4()` 永远不匹配 → 过滤失效
- `finish_reason` 无 `message_id` 过滤 → 旧请求的 stale finish 提前结束新流
- `editMessage` 先 cancel 清 `streamingMessageId=null` 再 send → 时序窗口内所有事件被丢弃
- coalescer/seqDedup 用 `message_id` 做 key → 旧请求和新请求的 seq 空间碰撞

### 11.3 对话分支模型

**来源**：I17b 对话分支

**从截断式改为分支式**：编辑/重试/重新生成操作不破坏性删除消息，改为非破坏性分支。

**已有基础设施**（可直接利用）：
- `parent_id` DB 列 ✅ 存在但从未使用
- `parentId` Rust/TS 字段 ✅ 存在

**需新增**：
- `sibling_index`：同一 `parentId` 下的兄弟排序
- `active_branch_id`：对话当前活跃分支指针
- `BranchPicker` UI 组件（`hideWhenSingleBranch` 模式，单分支时 DOM 不存在）

**分支上限**：20/parent（硬上限），达到上限禁止新建，提示 Fork 到新对话（降级策略）。

---

## 12 声明式设置与增量合并

### 12.1 声明式注册表模式

**来源**：I8 声明式设置系统

**问题**：新增一个设置项需改 8-12 文件（Rust struct → cmd → register → TS interface → event → listen → dispatch → UI）。

**解决方案**：声明式注册表 + `amend` 增量合并，新增设置项降至 1-2 文件。

```
Layer 0: Core（Rust amend + 校验）— patch_conf 统一增量命令
Layer 1: Registry（注册表 + 调度）— definitions + sideEffectRegistry + dispatcher
Layer 2: UI（设置页面）— 根据 definitions 自动渲染，支持 customRender 逃生舱
```

**`AppConf::amend` 增量合并**（借鉴 ChatGPT 参考项目）：
- 用 `BTreeMap` 合并 patch 字段到现有 config
- 合并后反序列化校验，类型不匹配立即报错
- 副作用通过 `sideEffectRegistry` 注册，`apply_side_effects` 遍历执行

### 12.2 设置项新增规范

**来源**：I8、项目规则

| 步骤 | 位置 | 内容 |
|------|------|------|
| 1 | Rust struct | 新增同名字段 + `#[serde(default)]`（前向兼容） |
| 2 | TS interface | `AppConf`/`OrbConf`/`MascotConf` 新增同名字段 |
| 3 | `definitions.ts` | `defineSetting()` 注册（类型 + 默认值 + 副作用） |

**禁止**：用 `Record<string, unknown>` 传递部分字段。

---

## 13 Provider 多 API 适配模式

### 13.1 分层架构

**来源**：I9 Provider 多 API 格式适配

```
assistant-providers/src/
├── base.rs              — trait 定义 + 错误类型 + 配置类型
├── client/              — HTTP 客户端基础设施（共享提取）
│   ├── http.rs          — 请求构建 + 发送 + 重试
│   ├── sse.rs           — execute_sse_request（SSE 请求执行入口）
│   ├── quirks.rs        — Provider 特异行为（URL 检测、reasoning 兼容、header 差异）
│   └── ...
├── types/               — 统一类型（UnifiedResponse + ChatDelta + ToolDefinition）
├── openai/              — OpenAIAdapter（请求/响应/SSE/错误分类/消息转换/模型发现）
├── anthropic/            — AnthropicAdapter
├── gemini/              — GeminiAdapter
└── stream/              — 流处理基础设施
```

### 13.2 关键设计原则

| 原则 | 说明 |
|------|------|
| **UnifiedResponse 统一响应** | 各 adapter 的响应转换为统一类型，上层不感知 Provider 差异 |
| **quirks 特异行为隔离** | URL 检测、reasoning 兼容、header 差异集中在 `quirks.rs`，不污染主逻辑 |
| **SSE 解析用 eventsource_stream crate** | 禁止手动 `buffer.find('\n')` 解析 |
| **流内错误检测** | 部分 provider 在流中返回错误，必须捕获 |
| **每 adapter 6 文件** | mod + request + types + stream + error/message_converter + discovery |

---

## 14 Tool 双入口架构

### 14.1 Agent 入口 vs ToolBox 入口

**来源**：I18 Agent Tool 架构、I18b ToolBox OS 交互桥接

两个入口共享 **Tool trait 实现**（实现一次，两个入口可用），但执行路径分流：

```
Tool trait 实现（共享）
  → ToolRegistry（共享：注册一次）
    ├── ToolBox 入口：os_tool_exec → ToolRegistry.find → Tool.execute() → 返回
    │   （直达执行，不经过 ToolPipeline）
    └── Agent 入口：RoundExecutor → ToolPipeline → ToolGateway（审批）→ Tool.execute() → ResultStorage（审计）
```

**为什么 ToolBox 不经过 ToolPipeline**：

| ToolPipeline 基础设施 | Agent 场景 | ToolBox 场景 |
|---|---|---|
| 审批流 | LLM 自主决策需人类审批 | 用户自己点的，审批自己无意义 |
| 权限策略评估 | 防止 LLM 越权 | 用户就是权限主体 |
| 审计日志 | 追踪 AI 自主行为 | 用户操作，非 AI 行为 |
| 结果回填消息历史 | Agent Loop 需继续对话 | 结果直接返回组件渲染 |

### 14.2 统一桥接 + 注册式扩展

**核心**：Rust 侧永远只有一个 `os_tool_exec` Tauri command，不管有多少个 OS 工具。

新增工具只需三步：
1. 实现 Tool trait + 注册到 ToolRegistry（Rust）
2. 前端 `os.ts` 加一个命名空间（TS）
3. React 组件 + `defineTool()`（UI）

**禁止**：为单个 OS 工具新增独立 Tauri command。

---

## 15 新功能引入检查总表

新功能文档编写或实施前，逐项确认：

### 15.1 架构层

- [ ] 是否遵循 Service → Orchestrator → Store 三层？
- [ ] Rust cmd 层是否为薄壳（零业务逻辑）？
- [ ] IPC 调用是否集中在 Service 层？
- [ ] Orchestrator 是否通过 Port 接口与 store 通信？
- [ ] 多实现模块是否采用 Core → Implementation → Orchestration 三层 + 注册表？
- [ ] 硬编码映射是否数据化（规则对象 + 求值逻辑）？

### 15.2 类型安全层

- [ ] 后端 command 返回值是否用 DTO struct（非 `serde_json::Value`）？
- [ ] DTO 是否加 `rename_all = "camelCase"` + `deny_unknown_fields`？
- [ ] Option 字段是否加 `skip_serializing_if` + `#[serde(default)]`？
- [ ] 前端是否删除 `as` 强转，改为泛型 `invoke<T>`？
- [ ] 类型定义是否独立文件（`xxx.types.ts`）而非与 Service 耦合？

### 15.3 事件层

- [ ] 新增 event 是否按三分类选择 emit 方式？
- [ ] event payload 是否有 Rust struct（非 `json!({})`）？
- [ ] 数据变更类 event 是否 `app.emit` 广播？
- [ ] 主窗口是否注册了 `listen()`（非仅靠轮询）？

### 15.4 错误处理层

- [ ] 是否用 `createLogger` 替代 `console.error`？
- [ ] 是否禁止 `.catch(() => {})` 静默吞没？
- [ ] Store 级错误是否用户可感知（通知系统）？
- [ ] React.lazy 是否包裹 ErrorBoundary？
- [ ] 错误是否有 category + recoveryActions（非仅 `retryable: boolean`）？

### 15.5 安全层

- [ ] 敏感操作是否走 Rust command（前端不接触明文）？
- [ ] 明文类型是否实现 Zeroize + Drop？
- [ ] 密钥轮换是否检查所有依赖旧密钥的派生数据？
- [ ] 锁定→解锁循环是否对称（SSH/biometric/emergency data）？

### 15.6 流式架构层

- [ ] 流式消费是否拆为 StreamAccumulator + consume_stream 双层？
- [ ] `into_outcome(self)` 是否消费 self（防半构造误用）？
- [ ] 流事件是否携带 `attempt_id`（非 `message_id`）？
- [ ] `attempt_id` 是否由后端生成（非前端造临时 ID）？
- [ ] 是否有 `chat-stream-start` 帧协议？
- [ ] 所有事件类型是否统一按 `attempt_id` 过滤？
- [ ] cancel→send 切换是否原子（无 `streamingMessageId=null` 时序窗口）？

### 15.7 Tool 架构层

- [ ] 新增 OS 工具是否通过 Tool trait + ToolRegistry 注册（非独立 Tauri command）？
- [ ] ToolBox 入口是否直达执行（不经过 ToolPipeline）？
- [ ] Agent 入口是否经过 ToolPipeline 审批流？
- [ ] 两个入口是否共享 Tool trait 实现？

### 15.8 风格对齐层

- [ ] 文件命名是否对齐已有先例（slice 接口、类型文件、子组件）？
- [ ] 目录组织是否对齐（cmd 目录 vs 单文件、slices 子目录 vs 平铺）？
- [ ] `pub use` re-export 是否保持外部 API 不变？
- [ ] `as unknown as` 是否仅在允许的 2 处？
- [ ] 新增设置项是否同步更新 interface + Rust struct + `defineSetting()`？

### 15.9 交付策略层

- [ ] 是否分阶段交付（每阶段独立可测）？
- [ ] 修复是否按风险分层（零风险→低→中→高）？
- [ ] 是否显式标注已砍掉阶段及理由？
- [ ] 拆分是否保持外部 API 不变？

---

## 16 参考项目速查

| 参考项目 | 核心借鉴点 | 借鉴到本项目的文档 |
|---------|-----------|------------------|
| **OpenBitFun** | Port 接口 + 工厂函数（orchestrator）、Permission Engine（tool 权限）、`createLogger` + console 劫持（日志）、`lazyWithRecovery`（懒加载容错）、DTO 集中定义(1个文件）、StreamContext + StreamProcessor（流式双层）、round_executor（attempt_id）、`app.emit` 广播 | I13、I15、I16、I17、I19、I18 |
| **OpenHuman** | 标记协议 + AttachmentStash（多模态）、`ToolPolicy` trait（工具策略）、`app.emit` 广播（事件）、BranchPickerPrimitive（分支选择器框架） | I2e、I17b、I18、I20 |
| **deepseek-harness-orb** | `LlmAttemptId` 独立 branded type、start/chunk/end 帧协议、settle/abandon 双路径 + max-tokens 截断 + tool 调度器 | I16、I15 |
| **AIRI** | `ApiError` 工厂方法（结构化错误）、image-projection 降级（非 Vision 模型图片处理）、response categoriser + transient retry | I19、I2e |
| **KeePassXC** | 屏幕锁/生物识别/TOTP/HIBP/SSH Agent/AutoType/Passkey | I9 |
| **Vaultwarden** | 条目级加密/紧急访问/Send 一次性分享/WebSocket 推送 | I9 |
| **prs** | Git 同步（prepare→操作→finalize）、Plaintext/Ciphertext 安全封装 | I9 |
| **ChatGPT** | `AppConf::amend` 增量合并模式 | I8 |

---

## 17 文档组织约定

| 目录 | 用途 | 文档性质 |
|------|------|---------|
| `docs/achievement/` | 已完成阶段（I1-I12） | spec + tasklist |
| `docs/achievement1/` | 已完成阶段（I13-I20） | spec + tasklist |
| `docs/store/` | 进行中/待实施的功能 | spec + tasklist + issues + deadcode + 拆分方案 |
| `docs/analysis/` | **分析指导文档** | 跨阶段综合分析，不作为执行计划 |
| `docs/designed/` | 顶层设计 | 架构级设计文档 |

**新功能文档编写顺序**：
1. `docs/analysis/` — 可行性分析（渗透地图 + 参考项目做法 + 影响评估）
2. `docs/store/` — 功能 spec（分阶段交付规格）+ tasklist
3. `docs/store/` — issues（实施中发现的问题）+ deadcode（死代码清理）+ responsibility-split（职责拆分）
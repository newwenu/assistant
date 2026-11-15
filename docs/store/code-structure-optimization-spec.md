# 代码组织与深度扁平度优化规格

| 项 | 值 |
|---|---|
| 状态 | 草案（待评审） |
| 日期 | 2026-11-15 |
| 范围 | `src/`（前端）、`src-tauri/src`、`src-tauri/crates/*/src`；不含 `tests/`、`examples/`、`dist/`、`target/`、生成物 |
| 口径 | 非空行（剔除纯空白行）。目录深度按仓库根起算，同时给出「crate/src 之后」的相对深度；扁平度按单个目录的直接子文件数计 |
| 关联 | [ARCHITECTURE.md](../ARCHITECTURE.md)（组件职责）、[CODE-STRUCTURE.md](../CODE-STRUCTURE.md)（现状事实，刷新要求见 §4 B5）、[CODE-SPLIT-PRINCIPLE.md](../principle/CODE-SPLIT-PRINCIPLE.md)（拆分判据唯一来源）、REFACTOR-SPLIT-PLAN（已归档，2026-11-07 执行计划）、[crate-architecture.md](./crate-architecture.md)（crate 边界，不在本文范围） |

> 本文回答五件事：代码放在哪（组织）、层次多深（深度）、同级文件是否过多（扁平度）、中间层是否必要（间接层）、数据与逻辑是否混装。
> **「什么时候把一个文件拆开」的判据不在本文重述**，唯一来源是 [CODE-SPLIT-PRINCIPLE.md](../principle/CODE-SPLIT-PRINCIPLE.md)；本文只补组织、深度、扁平度、间接层与数据混装五类判据。

---

## 0 与既有文档的关系

| 文档 | 关系 |
|---|---|
| `principle/CODE-SPLIT-PRINCIPLE.md` | 文件拆分、目录模块、依赖方向的唯一判据。本文所有条目都不得与之冲突 |
| REFACTOR-SPLIT-PLAN（已归档） | 2026-11-07 的执行计划。本文 §4 复核其全部未完成项并重新定级；两者冲突时以本文批次与目标结构为准，其 §2 已完成基线仍不得回退 |
| `CODE-STRUCTURE.md` | 只描述事实。本文 §2 的测量结果与它已有显著偏差（见 §2.10），落地后必须同步刷新 |
| `store/crate-architecture.md` | crate 划分与编译归属。本文**不增减 crate、不改依赖图** |

---

## 1 目标

| 编号 | 目标 | 判定方式 |
|---|---|---|
| S1 | 文件的职责数等于它需要响应的变化原因数 | 承接原则 §1；本文负责发现「长度未到 900 行但职责已混杂」的文件 |
| S2 | 非生成代码的目录深度不超过「域 / 子域 / 职责」三层（crate 内 `src/` 之后 ≤3 段） | 目录深度直方图 |
| S3 | 每个模块只有一个入口；不存在父子 barrel 重复同一导出清单；barrel 不含业务实现 | 依赖扫描 + 导出清单比对 |
| S4 | 静态数据与算法分离，单个逻辑文件不因数据表变长而变长 | 数据块占比 |
| S5 | `services/` 只放后端调用与传输，不放纯展示解析；`utils/` 不含业务语义 | import 图与调用面 |
| S6 | 单组件不同时承担「数据编排 + 表单/交互状态 + 布局」三类职责 | 组件内 hook 数、effect 数与 JSX 区块数 |
| S7 | 控制流嵌套 >4 层时抽返回值纯函数；JSX 嵌套 ≥13 层或组件 >400 非空行时按可见区块拆子组件 | 缩进深度直方图（仅作信号，不作门禁） |
| S8 | 拆分不改变对外行为、公开 API 与生成契约 | 契约零 diff + 测试通过 |
| S9 | 扁平目录按子域分组：直接子模块 ≥12 且存在 ≥2 个独立变化原因的子域（每个 ≥3 个文件）时建子目录；同一变化原因的 uniform peers 保持扁平 | 直接子文件计数 + 子域归类 |

---

## 2 现状测量（2026-11-15 快照）

### 2.1 规模

| 部分 | 文件数 | 非空行 |
|---|---|---|
| 前端 `src/`（不含测试） | 425 | 48,819 |
| 前端测试（`__tests__` / `*.test.*`） | 58 | 10,851 |
| Rust 子 crate 源码 | 254 | 52,736 |
| Rust 主 app 源码（`src-tauri/src` + `build.rs`） | 80 | 14,211 |
| Rust 测试（各 crate `tests/`、`src-tauri/tests`、`examples`） | 114 | 31,107 |

前端按顶层模块（不含测试）：

| 模块 | 文件 | 非空行 | 模块 | 文件 | 非空行 |
|---|---|---|---|---|---|
| `vault` | 67 | 9,566 | `history` | 17 | 2,614 |
| `tools` | 66 | 8,674 | `services` | 29 | 2,403 |
| `chat` | 76 | 8,589 | `stores` | 19 | 2,368 |
| `settings` | 63 | 7,112 | `mascot` | 24 | 2,054 |
| `components` | 13 | 1,516 | `orb` | 10 | 1,654 |

Rust 按包（`src/`，主 app 含 `build.rs`）：

| 包 | 文件 | 非空行 | 包 | 文件 | 非空行 |
|---|---|---|---|---|---|
| `assistant-tools` | 43 | 15,069 | `assistant-storage` | 21 | 4,042 |
| `assistant-app`（主 app） | 80 | 14,211 | `assistant-security` | 27 | 2,771 |
| `assistant-vault` | 54 | 9,774 | `assistant-sync` | 6 | 1,427 |
| `assistant-providers` | 42 | 9,277 | `assistant-crypto` | 6 | 1,071 |
| `assistant-agent` | 38 | 7,909 | `assistant-config` | 9 | 731 |
| `assistant-orb` / `assistant-crash` / `assistant-i18n` | 4 / 2 / 2 | 381 / 146 / 138 | | | |

事实：`assistant-tools` 已是**源码量第一**的包（原 `crate-architecture.md` 记录的 702 行已失效），主 app 与 `assistant-vault` 次之。

### 2.2 目录深度与扁平度

**深度**（`src/` 之后）：

| 侧 | 深度分布 | 最大深度 |
|---|---|---|
| 前端 | 1 段 19 个目录、2 段 64、3 段 31、4 段 1 | 4 段，仅生成物 `vault/core/contract/serde_json/` |
| Rust | crate 内 1 段 30 个目录、2 段 7 | 2 段 |

**扁平度**（单个目录的直接子文件数，含 `mod.rs` / `index.ts`，不含测试与生成物）：

| 侧 | ≥10 个直接子文件的目录 |
|---|---|
| 前端 | `services` 24、`stores` 18、`chat/message` 18、`chat/core` 17、`vault/settings` 15、`tools` 14、`settings/core` 13、`vault/core` 13、`vault` 12、`chat/tool` 11、`orb` 10、`settings/core/types` 10 |
| Rust | `src-tauri/src/cmd` 19、`assistant-tools/src/os` 14、`src-tauri/src` 13、`adapters` 13、`assistant-tools/src` 13、`assistant-storage/src` 13、`assistant-agent/src/kernel` 13、`assistant-vault/src/dto` 12、`src-tauri/src/protocol/dto` 12、`unlock_dialog/windows` 11、`assistant-agent/src` 11 |

结论：**深度健康，扁平度是当前的主要结构问题**。判断分三种情况：

1. 若目录内文件全部响应同一类变化（`cmd` 的域入口、`adapters` 的端口实现、`protocol/dto` 的域 DTO、`unlock_dialog/windows` 的平台实现、各 crate `tests/`、23 个工具目录、`settings/core/types` 的类型文件），扁平是正确形态，不分组（R11）。
2. 若目录内可按**独立变化原因**归入 ≥2 个子域、且每个子域 ≥3 个文件，则判定为过扁，按子域建子目录（B4）。
3. `foo.rs` 与同名目录 `foo/` 并存是另一类问题，与扁平度无关（§2.4）。

深度不是问题，因此 B4 的分组**只增加语义子目录，不增加无意义的中间层**：分组后仍在 S2 的 3 段上限内（当前非生成代码最深 3 段，分组后最多到 3 段）。

### 2.3 逻辑嵌套

| 侧 | 最深 | ≥10 层文件数 |
|---|---|---|
| Rust | 13 层：`assistant-providers/src/openai/responses_stream.rs`；12 层：`anthropic/request.rs`；11 层：`security/src/screen_lock/windows.rs`、`os/agent_ask_user.rs`（后者为 JSON schema 字面量） | 9 |
| 前端 | 16 层：`settings/provider/ConnectDialog.tsx`；15 层：`settings/shared/sections/PermissionAuditSection.tsx`；14 层：`tools/text-diff/index.tsx`、`chat/tool/ToolConfigPanel.tsx` | 36 |

说明：JSX 缩进天然较深，缩进本身不是缺陷判据；只有「≥13 层且单组件 >400 非空行」时，才视为需要按可见区块拆子组件的信号（S7）。Rust 侧深缩进集中在 `async_stream::stream!` 的 `match` 内，处理方式按原则 §5「宏边界不可搬移」执行。

### 2.4 间接层（入口与重复导出）

| 现象 | 位置 | 事实 |
|---|---|---|
| 死 barrel | `src/mascot/core/index.ts`、`src/settings/core/types/index.ts` | 全仓无任何 import 解析到这两个文件（消费方都直接引用子模块） |
| 重复导出清单 | `src/mascot/index.ts` 与 `src/mascot/core/index.ts` | `core/index.ts` 的全部导出在 `mascot/index.ts` 中再列一遍，改一处需同步两处 |
| 文件与同名目录并存 | `assistant-agent/src/kernel/ports.rs` + `ports/test_doubles.rs`；`assistant-tools/src/os/agent_read.rs` + `agent_read/window.rs` | 与原则 §4「`foo.rs` → `foo/mod.rs`」不一致，且让模块深度多算一层 |

### 2.5 放置错位

| 现象 | 位置 | 判定 |
|---|---|---|
| 传输层放纯展示逻辑 | `src/services/auditDetail.ts`（153）、`src/services/auditLexicon.ts`（111） | 二者零 `invoke`，只做事件词表与 `detail` 文法解析；消费方是 `vault/settings/SecurityLogTab.tsx` 与 `settings/shared/sections/PermissionAuditSection.tsx`，违反 `CODE-STRUCTURE.md` §3.4 对 `services/` 的边界定义 |
| 通用目录放业务函数 | `src/utils/formatConversationTitle.ts` | 会话标题格式化是业务规则；`utils/` 其余 5 个文件为格式化、懒加载、模块载入、持久化队列，属通用设施 |
| 共享设施与工具实现同层 | `assistant-tools/src/os/` | `path.rs`（983）、`output.rs`（479）、`text_encoding.rs`（455）被 10 个工具共用，却与 10 个工具文件平铺在同一目录（17 个模块） |

### 2.6 数据与逻辑混装

`src/tools/password-generator/password-generator.service.ts` 共 2,159 非空行，其中第 137 行起是约 2,000 行的 EFF 大词表，词表之后只剩 `generatePassphrase` 等少量函数。这是全仓唯一一处「数据表主导文件长度」的样本。

### 2.7 Rust 单文件多职责清单（`src/` 内 ≥400 非空行）

| 文件（非空行） | 变化原因 | 目标结构 |
|---|---|---|
| `assistant-tools/src/gateway.rs`（1170） | 审批域类型 / 网关决策 / 审批上下文构建 / 授权与审计存储 / 审批键归一化 / 测试替身 | `gateway/{mod,types,decision,context,stores,approval_key,testing}.rs` |
| `assistant-tools/src/os/path.rs`（983） | 路径归一化 / 受保护目录与凭据目录 / 工作区归属 / 参数解析 | `os/path/{mod,resolve,protected,membership,args}.rs` |
| `assistant-tools/src/mcp/stdio.rs`（937） | JSON-RPC 帧编解码 / 客户端与挂起表 / 超时与生命周期 / stderr 日志 | `mcp/stdio/{mod,client,transport,codec,stderr}.rs` |
| `assistant-tools/src/os/agent_delete.rs`（899） | 安全边界 / 预扫描与预览 / 删除执行 / 回收站与卷判定 | `os/agent_delete/{mod,target,attrs,scan,exec,trash}.rs` |
| `assistant-tools/src/compact.rs`（836） | 压缩结构 / 头部渲染 / 锚点构建 / 截断解析 / 参数预览 | `compact/{mod,types,head,anchor,truncation,preview}.rs` |
| `assistant-tools/src/mcp/manager.rs`（781） | 配置与状态类型 / 生命周期与重连 / 连接抽象 | `mcp/manager/{mod,types,lifecycle}.rs` |
| `assistant-app` `cmd/conversation.rs`（762） | 列表 / 详情与 DTO 组装 / 分页 / 变更 / 分支 / 导出 | `cmd/conversation/{mod,list,detail,detail_dto,mutate,branch,export}.rs` |
| `assistant-app` `protocol/dto/chat.rs`（757） | 会话 / 消息 / 用量 / 事件载荷 / 内容块 | `dto/chat/{conversation,message,usage,event,content}.rs` |
| `assistant-tools/src/os/agent_grep.rs`（752） | 请求解析 / 遍历策略 / sink 接线 / 非 UTF-8 探测 / 输出渲染 | `os/agent_grep/{mod,request,outcome,search,sink,probe,render}.rs` |
| `assistant-tools/src/mcp/bridge.rs`（731） | 工具命名 / 工具定义 / 调用与结果映射 | `mcp/bridge/{mod,naming,defs,call}.rs` |
| `assistant-vault/src/vault_file/mod.rs`（714） | 条目文件读写 / `vault_meta` 签名校验 / 原子写 / 导入解码 | `vault_file/{mod,entry,meta,emergency}.rs` |
| `assistant-agent/src/multimodal.rs`（692） | 配置与信任级别 / 校验 / 准入 / 重水化 / 投影 / 文本回退 | `multimodal/{mod,validate,blocks,admit,rehydrate,project,text}.rs` |
| `assistant-providers/src/openai/responses_stream.rs`（688） | 重试 / SSE 解析 / 事件分支 / 工具调用累积 | `responses_stream/{mod,state,event_handler,in_progress,helpers}.rs` |
| `assistant-app` `cmd/permission.rs`（652） | 授权 CRUD / 审计写入 / 审计查询 / DTO 映射 | `cmd/permission/{mod,grants,audit}.rs` |
| `assistant-vault/src/security_db/mod.rs`（644） | 状态容器 / 会话与 DEK 守卫 / 锚点期望 / 备份与 pragma | `security_db/{mod,state,session,anchor,schema}.rs` |
| `assistant-app` `adapters/question.rs`（580） | 待决注册表 / 答案语义校验 / 提示器 / DTO | `adapters/question/{mod,registry,prompter,dto}.rs` |
| `assistant-providers/src/gemini/message_converter.rs`（532） | 消息 / 工具 / schema 转换 | `message_converter/{messages,tools,schema}.rs` |
| `assistant-sync/src/git_sync/git.rs`（531） | 仓库 / 分支 / 远端 / 日志 / 底层命令 | `git_sync/git/{mod,repo,branch,remote,log,plumbing}.rs` |
| `assistant-tools/src/os/agent_write.rs`（523） | 编码策略 / 原子写 / 行数统计 / 审批预演 | `os/agent_write/{mod,encoding,atomic,lines,preview}.rs` |
| `assistant-vault/src/dto/convert.rs`（472） | sync / entry / totp / error / hibp 五类 `From` | `dto/convert/{sync,entry,totp,error,hibp}.rs` |
| `assistant-providers/src/anthropic/message_converter.rs`（463） | 用户 / 助手 / 工具结果消息转换 | `message_converter/{messages,tools}.rs` |

上述条目的判据、风险与「先补 fixture 再拆」要求，沿用 REFACTOR-SPLIT-PLAN（已归档）§1.1 与 §1.2 的结论，本文只调整优先级与目标结构。

### 2.8 前端单文件/单组件职责清单（非空行 ≥400；`ConnectDialog` 按嵌套信号纳入）

| 文件（非空行） | 现状 | 目标 |
|---|---|---|
| `src/tools/password-generator/password-generator.service.ts`（2159） | 词表 + 算法 | `effWordlist.data.ts` + 保留算法 |
| `src/settings/shared/sections/PermissionAuditSection.tsx`（624） | 拉取与翻页 / 列宽拖拽 / 统计计算 / 表格布局 | `permissionAudit/{useAuditPaging,useAuditColumns,auditStats,PermissionAuditTable}.ts(x)` |
| `src/vault/settings/VaultSyncSettings.tsx`（617） | 25 个 store 选择器 + 3 个 effect + 表单状态 + 多区块布局 | `vaultSync/{useVaultSyncForm,SyncConfigSection,SyncActionsSection,SyncResultSection}.tsx` |
| `src/vault/settings/VaultConflictResolve.tsx`（606） | 预览加载 / 揭示计时 / 解决动作 / 对比表 | `vaultConflict/{useConflictPreview,ConflictCompareTable}.tsx` |
| `src/services/stream/turnOrchestrator.ts`（543） | 编排 / 阶段映射 / 标题回退工具 | 编排保留；`phase.ts`、`title.ts` 抽出 |
| `src/settings/pages/AgentModeSettings.tsx`（540） | 头像组件 + 表单数据 + 页面 | `agentMode/{AvatarPreview,useModeForm}.tsx` |
| `src/chat/message/MessageList.tsx`（478） | 滚动跟随、前插补偿、未读计数等状态与列表渲染同处 | 抽 `useMessageScroll.ts`（跟随判定与补偿），列表只保留装配 |
| `src/mascot/renderers/svg/GhostyMascot.tsx`（462） | 面部预设表 + 路径算法 + 渲染 | `ghosty/{facePresets,paths}.ts` + 渲染组件 |
| `src/settings/provider/ConnectDialog.tsx`（383） | 16 层 JSX 嵌套；字段 schema 分支、提交逻辑与字段渲染内联 | 抽 `useConnectForm` + 字段区块子组件 |

其余 ≥400 行文件本轮不列为目标：`ConversationView` 是已确定的装配点，`chatConversationSlice` / `vaultSyncSlice` 是状态切片，`snapshot.ts` 是单一「快照契约」流水线；`VaultEntryDetail` / `OrbDrag` 尚未出现第二个变化原因。重新评估的触发条件见 §3 R9。

### 2.9 复核后判定「不动」的项（避免反复扫描）

| 文件 | 结论 |
|---|---|
| `assistant-agent/src/kernel/ports/test_doubles.rs`（801） | 测试替身单一职责，且被跨 crate 集成测试当公开 API 使用；不属原则 §3 的内联测试外移对象 |
| `assistant-agent/src/agent/middleware.rs`（524） | 7 个中间件共用一条链，变化原因唯一（中间件链语义） |
| `assistant-storage/src/db.rs`（583） | 单一「建库与迁移链」职责；随迁移增长不构成拆分理由 |
| `assistant-tools/src/policy.rs`（468）、`os/output.rs`（479）、`os/text_encoding.rs`（455） | 各自单一职责，仅长度靠前 |
| `assistant-security/src/unlock_dialog/windows/wm_handlers.rs`（347） | Phase C 拆分后的正常子模块 |
| 前端 `chat/ConversationView.tsx`、`stores/*Slice.ts`、`chat/core/*.ts` | 已按装配点 / 切片 / 纯逻辑分离，符合现状约定 |

### 2.10 文档基线漂移

`CODE-STRUCTURE.md` 与 `crate-architecture.md` 的数字和模块清单已落后于实测：

| 项 | 文档值 | 实测 |
|---|---|---|
| `assistant-tools` | 702 行 / 8 文件 | 15,069 行 / 43 文件（另含 `mcp/`、`compact.rs`、`policy.rs`、`pipeline.rs` 等文档未列模块） |
| `assistant-agent` | 2,192 行 / 13 文件 | 7,909 行 / 38 文件（含 `kernel/`、`agent/`） |
| `assistant-vault` | 9,143 行 / 47 文件 | 9,774 行 / 54 文件 |
| 主 app | 6,702 行 / 39 文件 | 14,123 行 / 79 文件 |
| Rust 合计 | 37,903 行 | 源码 66,947 行（不含测试） |

这些偏差会让后续结构判断建立在错误的规模认知上，属本规格的附带交付（§4 B5）。B5-1 已按上表刷新 `CODE-STRUCTURE.md`，上表保留为刷新前快照；B5-2 / B5-3 待做。

---

## 3 规则

> 原则 §1–§7 不在此复述。以下每条都是可操作要求，编号供批次条目引用。

| 编号 | 规则 | 操作化检查 |
|---|---|---|
| R1 | **传输层纯净**：`services/` 不出现纯展示解析、词表、文案映射；此类逻辑放消费域或独立领域模块 | `services/` 下文件不得只有 i18n / 纯函数 import |
| R2 | **通用层无业务**：`utils/` 只放不依赖业务类型的函数；业务格式化进对应领域 `core/` | 该文件 import 是否出现 `Conversation` / `Vault` 等业务类型 |
| R3 | **目录模块统一**：需要子模块时一律 `foo/mod.rs`，禁止 `foo.rs` + `foo/` 并存 | `Test-Path foo` 与 `foo.rs` 同时为真即违规 |
| R4 | **单一入口**：一个模块只有一个对外入口；父子目录不得各维护一份相同导出清单；barrel 只 re-export | 导出名集合比对；`index.*` 内不得有业务实现（组件族唯一入口除外）；ESLint 门禁（`no-restricted-syntax`）禁止显式 `/index` 引用、域内文件引用自身 barrel、`index.*` 内 `export *` |
| R5 | **数据与逻辑分离**：≥200 行的静态表独立为 `*.data.ts`，不得与算法同文件 | 单文件内连续字面量占比 |
| R6 | **深度上限**：非生成代码在 `src/` 之后不超过 3 段；不新增只做转发的中间目录 | 目录深度直方图 |
| R7 | **组件三类职责分离**：数据编排（store/service 调用）、表单与交互状态、布局渲染，三者同处一个组件时必须拆 hook 或区块组件 | 组件内 store 选择器数、effect 数、JSX 区块数 |
| R8 | **嵌套信号**：Rust 控制流 >4 层抽返回值纯函数；前端 JSX ≥13 层且组件 >400 行按可见区块拆 | 缩进深度直方图（信号，不作门禁） |
| R9 | **观察项触发**：列入 §2.8 末段与 §2.9 的文件，只有出现第二个变化原因时才重新评估；**不因行数增长自动触发** | 改动时在提交说明写明新增变化原因 |
| R10 | **一次提交只做一件事**：纯搬移与语义改写分开；搬移提交不得顺手改文案、错误类型或公开签名 | diff 中是否同时出现 `mv` 与行为修改 |
| R11 | **扁平度**：目录直接子模块 ≥12 时审查；子域必须对应**独立变化原因**且 ≥3 个文件才建子目录；机械按位置 / 类型（`rows` / `part1`）分组不算子域；uniform peers 保持扁平；改动面显著大于收益时可暂缓，但须在 B4 记录理由 | 直接子文件计数 + 变化原因归类 |
| R12 | **分组纯度**：子目录名对应稳定子域（`sync` / `recovery` / `shared` / `system`），不按数量或 `part1`/`part2`；分组是纯搬移，import 同步更新，不留兼容 shim | 目录名 + diff |

---

## 4 批次

排序原则：零风险清理 → 放置归位 → Rust 拆分 → 前端拆分 → 扁平目录分组 → 文档刷新。批次内先纯搬移，再抽函数骨架；需要新状态结构的（`responses_stream`、`multimodal`）独立提交并先补 fixture。

### B0 零风险清理

| 条目 | 动作 | 验收 | 状态 |
|---|---|---|---|
| B0-1 | 删除死 barrel：`src/mascot/core/index.ts`、`src/settings/core/types/index.ts` | `npm run typecheck` 通过 | ✅ 2026-11-15 |
| B0-2 | 消除重复导出：`src/mascot/index.ts` 直接 re-export 子模块，不再经 `core/index.ts` | 导出名集合不变 | ✅ 2026-11-15（随 B0-1 自动达成） |
| B0-3 | `kernel/ports.rs` → `kernel/ports/mod.rs`；`os/agent_read.rs` → `os/agent_read/mod.rs` | 模块路径不变，`cargo check` 通过 | ✅ 2026-11-15 |
| B0-4 | EFF 词表外移为 `tools/password-generator/effWordlist.data.ts` | 生成结果与密码强度测试不变 | ✅ 2026-11-15 |

### B1 放置归位

| 条目 | 动作 | 依据 | 状态 |
|---|---|---|---|
| B1-1 | `services/auditDetail.ts`、`services/auditLexicon.ts` 移入 `src/audit/`（`detail.ts` / `lexicon.ts`），更新两个消费方与测试 | R1 | ✅ 2026-11-15 |
| B1-2 | `utils/formatConversationTitle.ts` 移入 `history/core/`（会话展示域），更新 `orb` 与 `history` 三处 import | R2 | ✅ 2026-11-15 |

> B1-1 采用独立领域目录而非塞进 `settings/` 或 `vault/`：两者都要用，任一方承载都会造成反向依赖。落地后 `src/audit/` 只含 `detail.ts`、`lexicon.ts` 与 `__tests__/`，不建 barrel（消费方直连文件）。

### B2 Rust 拆分

次序：**先共享叶子下沉，再按文件拆**。`assistant-tools` 的三处重复（`base_directory`、`WalkBuilder` 配置、`ensure_empty_directory`）必须先按 REFACTOR-SPLIT-PLAN（已归档）§1.2 下沉到 `os/{scope,walk,fs_util}.rs`，否则拆完只是把第二份拷贝搬进子模块。

| 优先级 | 条目 | 依赖 |
|---|---|---|
| P1 | `gateway.rs`、`os/path.rs`、`os/agent_delete.rs`、`compact.rs`、`mcp/stdio.rs` | 无；纯搬移为主 |
| P1 | `cmd/conversation.rs`、`protocol/dto/chat.rs`、`cmd/permission.rs`、`security_db/mod.rs` | 无 |
| P2 | `os/agent_grep.rs`、`os/agent_write.rs`、`mcp/manager.rs`、`mcp/bridge.rs`、`vault_file/mod.rs`、`dto/convert.rs` | 共享叶子下沉完成 |
| P2 | `gemini/message_converter.rs`、`anthropic/message_converter.rs`、`git_sync/git.rs`、`adapters/question.rs` | 无 |
| P3 | `multimodal.rs`、`openai/responses_stream.rs` | 需新状态结构；先补 fixture，独立 PR |

`cmd/chat/insert.rs`（569）不进入本规格：其阻塞条件（5 处 `assistant_i18n::t()` 需先改为结构化错误）仍未解除，维持 REFACTOR-SPLIT-PLAN（已归档）的结论。

### B3 前端拆分

| 优先级 | 条目 | 依赖 |
|---|---|---|
| P1 | `PermissionAuditSection`、`VaultSyncSettings`、`VaultConflictResolve` | 三者为「编排 + 状态 + 布局」混合，拆分前先补关键交互手测清单 |
| P2 | `AgentModeSettings`、`MessageList`、`GhostyMascot`、`ConnectDialog` | B0-4 完成后 |
| P3 | `turnOrchestrator`（仅抽 `phase.ts` / `title.ts`） | 与 `services/stream` 测试同步 |

### B4 扁平目录分组

前置：B0–B3 完成后执行，避免与文件拆分并发修改同一批文件。全批为**纯搬移**：子目录名对应稳定子域，import 同步更新，不留兼容 shim，不改行为。

分组判据见 R11：直接子模块 ≥12 时审查；只有子域对应独立变化原因、且每个子域 ≥3 个文件才分组。

| 优先级 | 目录（直接文件数） | 子域与目标 | 备注 |
|---|---|---|---|
| P1 | `src/vault/settings`（15） | `sync/`（3：VaultSyncSettings / VaultConflictResolve / SshAgentTab）、`recovery/`（5：RecoveryKeyDialog / RecoveryKeyForm / RecoveryTab / RotateDekDialog / printPaperKey）、`security/`（5：ChangePasswordTab / SecurityLogTab / DangerZone / DangerZoneConfirm / DangerZoneTab）；`VaultSettings`、`ImportExportTab` 留根层 | 先完成 B3 对 VaultSyncSettings / VaultConflictResolve 的拆分，再搬移 |
| P1 | `src/tools`（14） | `framework/`（6：registry / types / categories / capabilities / os / miniNavigate）、`ui/`（8：ToolBox / MiniToolBox / ToolPage / ToolCard / ToolSearchBar / FavoriteButton / CollapsibleToolMenu / ToolCapabilityWrapper）；23 个工具目录不动 | `registry.ts` 内 23 条 `import("./<tool>")` 需改为 `../<tool>`，导入面集中在 `tools/` 与少量设置页 |
| P1 | `src/chat/core`（17） | `stream/`（3：streamCoalescer / useTypewriter / optimisticMessage）、`message/`（4：messageTree / contentBlockTypes / contentBlockUtils / conversationLink，纯逻辑消息模型，区别于 UI 层 `chat/message/`）；其余 10 个文件留根层 | 其余子域各 2 个文件，未达 3 个，不再细分 |
| P2 | `src/services`（24） | `chat/`（3）、`tool/`（3：toolService / permissionService / mcpService）、`system/`（5：tauriInvoke / crossWindowEvent / logger / globalErrorHandler / reactProductionError）、`desktop/`（4：storageService / workspaceService / orbService / windowService）；`history` / `settings` / `model` 各 2 个留根层 | B1 移出 `audit*` 后剩 22 个；分组后根层约 7 个单文件 |
| P2 | `src/stores`（18） | `chat/`（6：chatStore / chatConversationSlice / chatMessageSlice / chatCompactSlice / chatStorePorts / chatStreamStore）、`system/`（4：themeStore / localeStore / notifyStore / windowModeStore）；`agentModeStore` 并入已有 `agent/` | 其余 7 个领域 store 留根层 |
| P2 | `assistant-tools/src/os`（14 + 3 目录） | `tools/`（9 个工具文件 + `agent_edit/` / `agent_read/` / `fs_link/`）、`shared/`（path / output / text_encoding / process_output） | 触碰 17 个文件的 26 处 `crate::os::*` 引用；`os/mod.rs` 保留 `pub use` |
| P2 | `src/settings/core`（13） | `registry/`（5：builtinRegistry / definePage / initSettingSystem / pageRegistry / sideEffectRegistry）、`runtime/`（8：confRuntime / snapshot / settingsDispatcher / optimisticUpdate / query / insertUnique / useConfGroup / useConfPatchHandlers）；`types/` 已存在 | 导入面集中在 `settings/` 与 `features/` |
| P3 | `src/chat/message`（18） | 暂不分组：18 个文件同属「消息行渲染」一个变化原因；仅当 `input/` 或 `scroll/` 出现独立变化时再建子目录 | 观察项，触发条件见 R11 |
| P3 | `src/vault/core`（13） | `hooks/`（4：useVaultAutoLock / useVaultEvents / useVaultStatusReady / useVaultUnlock）、`display/`（6：paperKey / syncDisplay / vaultError / vaultErrorText / vaultColors / vaultCategory）；`vaultStore` / `vault.service` / `vault.types` 留根层 | 与 `slices/`、`contract/` 同级，分组后仍在 3 段内 |
| P3 | `src/vault`（12） | 暂不分组：列表 / 外壳 / 抽屉同属「保险库页面装配」一个变化原因 | 页面装配出现独立子变化时再评估 |
| — | `src/chat/tool`（11）、`src/orb`（10） | 未达 12 的审查阈值，不分组 | — |
| P3 | `assistant-storage/src`（13） | `tools/`（3：tool_history / tool_favorites / mcp_servers）；`db` / `migrations` / `attachment_store` / `content_codec` 等各 1–2 个文件留根层，不建目录 | 触碰各 crate 调用点，收益中等 |
| P3 | `assistant-agent/src/kernel`（13） | `turn/`（runner / phase / state / outcome / emit / persist / stream）；`ports/`、`request`、`tasks`、`limits`、`ids` 留根层 | 与 `kernel` 公开路径同步 |
| P3 | `assistant-tools/src`（13） | `contract/`（trait / definition / context / result）、`runtime/`（gateway / policy / exec / pipeline / registry / compact / question / storage） | `lib.rs` 的 `pub use` 路径保持不变 |
| P3 | `src-tauri/src/cmd`（19） | 暂不分组：多数文件本身即域入口，分组需同步 `cmd/mod.rs` 与 `ipc.rs`，改动面大于收益 | 观察项 |

**保持扁平（明确不动）**：`src-tauri/src` 根的 13 个装配入口、`adapters/` 13 个端口实现、`protocol/dto/` 12 个域 DTO、`assistant-vault/src/dto/` 12 个域 DTO、`unlock_dialog/windows/` 11 个平台实现、各 crate `tests/`、23 个 `src/tools/*` 工具目录、`src/settings/core/types/` 的类型文件。这些目录的文件是 uniform peers：同一变化原因、互相平级、没有可命名的子域。

`os/` 分组是原 B4 的正式化：不再按「模块数 ≥20」延后，而是作为 P2 执行；若在 B2 拆分 `os/agent_*` 文件时已建立子目录，则在同一 PR 内顺带完成 `shared/` 下沉，避免二次搬移。

### B5 文档刷新

| 条目 | 动作 | 状态 |
|---|---|---|
| B5-1 | 更新 `CODE-STRUCTURE.md` 的模块清单（补 `mcp/`、`kernel/`、`agent/`、`sandbox/`）、脚本清单、测试布局与定位索引 | ✅ 2026-11-15 |
| B5-2 | 更新 `crate-architecture.md` 各 crate 行数/文件数与模块列表 | 待做 |
| B5-3 | REFACTOR-SPLIT-PLAN 已移入 `docs/achievement/`（待归档），不再需要顶部标注 | ✅ 2026-11-15（作废） |

---

## 5 验收

每个批次完成时：

```
# Rust（工作目录 src-tauri）
cargo check
cargo test
cargo clippy --all-targets -- -D warnings

# 前端（仓库根）
npm run typecheck
npm run format:check
npx eslint . --max-warnings 0
npm test

# 契约（仅改动 DTO / 配置组时）
npm run contract:check
```

`assistant-vault` / `assistant-crypto` 的门禁特性开关与慢测命令见 REFACTOR-SPLIT-PLAN（已归档）§3，此处不复述。

结构验收（人工）：

1. S2 / R6：新增目录后重跑目录深度统计，`src/` 之后不得出现第 4 段（生成物除外）。
2. S9 / R11：重跑直接子文件统计；分组后原目录应 ≤11 个直接子文件，新子目录内不得残留「≥3 个同子域文件仍平铺」的遗漏。
3. S3 / R4：全仓搜索是否存在未被引用的 `index.*`；父子 barrel 的导出名集合必须无重复；显式 `/index` 引用、域内引用自身 barrel、`index.*` 内 `export *` 由 ESLint 直接报错（`npm run lint`）。
4. S5 / R1–R2：`services/` 与 `utils/` 的文件 import 检查。
5. S8：`git diff` 中不得出现 `pub use` 路径、DTO 字段或生成契约的变化。

无自动化测试覆盖的模块（`multimodal`、`unlock_dialog`）除上述命令外必须补手测：附件四类场景与解锁流程，清单见 REFACTOR-SPLIT-PLAN（已归档）§3。

---

## 6 不做的事

- 不新增 crate、不调整依赖方向与 feature 划分（属 `crate-architecture.md` §5 的判据）。
- 不以行数为拆分目标；不把测试当拆分理由。
- 不重命名公开 API、不改 DTO 字段、不手改生成契约。
- 不把所有 `index.ts` 一律取消：模块边界入口保留，只删除重复与死 barrel。
- 不为「更扁平」而压平有意义的子域目录（如 `vault/core/slices`）。
- 不为降低同级文件数而强行分组 uniform peers（`cmd` 域入口、`adapters` 端口、`dto` 域文件、23 个工具目录、`settings/core/types`）；分组必须有可命名的子域。
- 不在同一个提交里混合搬移与语义改写。

---

## 7 风险与回退

| 风险 | 影响 | 处置 |
|---|---|---|
| `multimodal` / `responses_stream` 无测试，拆分类语义漂移 | 附件与流式增量行为变化 | 先补 fixture，纯搬移与改写分开提交 |
| 前端组件拆分改变视觉或交互 | 回归面广 | 拆分前记录关键状态截图与手测清单；只搬 JSX 不改样式 |
| `gateway.rs` 拆分时类型与归一化规则分离 | `ApprovalKey` 规则分散导致双向 `use super` | 类型与归一化规则同处一个子模块（沿用原结论） |
| 扁平分组的搬移面大，import 漏改 | 编译错误或运行期模块解析失败 | 一次只分组一个目录；搬移提交用 `npm run typecheck` / `cargo check` 收口；不留 shim |
| 文档再次漂移 | 后续判断失真 | B5 与结构改动同批提交，禁止只改代码不更新事实文档 |

---

## 8 复现命令

```powershell
# 非空行
function NB($p){ $n=0; foreach($l in [System.IO.File]::ReadLines($p)){ if($l.Trim().Length -gt 0){$n++} }; $n }

# 前端各模块规模（src 之后深度 = 路径段数 - 1）
Get-ChildItem src -Recurse -File -Include *.ts,*.tsx |
  Where-Object { $_.FullName -notmatch 'node_modules|__tests__|\.test\.' } |
  ForEach-Object { [PSCustomObject]@{ Top=($_.FullName -split '\\src\\')[1].Split('\\')[0]; NB=(NB $_.FullName) } } |
  Group-Object Top |
  ForEach-Object { "{0}|files={1}|nonblank={2}" -f $_.Name,$_.Count,(($_.Group|Measure-Object NB -Sum).Sum) }

# 目录深度（src 之后）
Get-ChildItem src -Recurse -Directory |
  ForEach-Object { (($_.FullName -split '\\src\\')[1] -split '\\').Count } |
  Group-Object | Sort-Object Name

# 扁平度（直接子文件数 >=10；前端）
Get-ChildItem src -Recurse -Directory |
  ForEach-Object {
    $n = (Get-ChildItem (Join-Path $_.FullName '*') -File -Include *.ts,*.tsx -ErrorAction SilentlyContinue).Count
    if ($n -ge 10) { "{0}|files={1}" -f $_.FullName, $n }
  }

# 扁平度（直接子文件数 >=10；Rust，排除生成/测试目录）
Get-ChildItem src-tauri -Recurse -Directory |
  Where-Object { $_.FullName -notmatch '\\target\\|\\tests\\' } |
  ForEach-Object {
    $n = (Get-ChildItem (Join-Path $_.FullName '*') -File -Include *.rs -ErrorAction SilentlyContinue).Count
    if ($n -ge 10) { "{0}|files={1}" -f $_.FullName, $n }
  }

# 逻辑嵌套深度（Rust，缩进 /4；前端缩进 /2）
Get-ChildItem src-tauri -Recurse -File -Include *.rs |
  Where-Object { $_.FullName -notmatch '\\target\\|\\tests\\' } |
  ForEach-Object {
    $max=0
    foreach($l in [System.IO.File]::ReadLines($_.FullName)){
      if($l.Trim().Length -eq 0 -or $l.TrimStart().StartsWith('//')){ continue }
      $m=[regex]::Match($l,'^[ \t]*').Value.Length; if($m -gt $max){$max=$m}
    }
    [PSCustomObject]@{ Depth=[math]::Floor($max/4); Path=$_.FullName }
  } | Sort-Object Depth -Descending
```

---

## 9 定位索引

| 需求 | 位置 |
|---|---|
| 新增前端领域模块 | 顶层 `src/<domain>/`，入口只放 barrel；实现按 `core/`（纯逻辑）、UI 目录、`slices/`（状态）分层 |
| 新增前端跨域展示逻辑 | 独立领域目录（如 `src/audit/`），禁止放 `services/` 或让一方领域依赖另一方 |
| 新增静态大表 | `<tool>/<name>.data.ts`，与算法文件分离 |
| 新增 Rust 子模块 | `foo.rs` → `foo/mod.rs`；共享叶子下沉到被依赖方都不依赖的独立模块 |
| 某目录同级文件增多 | 先按 R11 归类：≥2 个独立变化原因的子域且各 ≥3 个文件才建子目录；同一变化原因的 uniform peers 保持扁平 |
| 新增工具实现 | `assistant-tools/src/os/<tool>.rs`；出现第二个变化原因时再升级为目录模块 |
| 判断是否该拆 | `principle/CODE-SPLIT-PRINCIPLE.md` |
| 判断放哪个 crate | `store/crate-architecture.md` §5 |
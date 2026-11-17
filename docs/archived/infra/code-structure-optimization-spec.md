# 代码组织与深度扁平度优化规格

| 项 | 值 |
|---|---|
| 状态 | 已归档（冻结，2026-11-16） |
| 日期 | 2026-11-16 |
| 范围 | `src/`（前端）、`src-tauri/src`、`src-tauri/crates/*/src`；不含 `tests/`、`examples/`、`dist/`、`target/`、生成物 |
| 口径 | 非空行（剔除纯空白行）。全部数字由 `scripts/probe-structure.mjs` 生成（见 §8），不手工统计；扁平度按单个目录的直接子文件数计 |
| 关联 | [ARCHITECTURE.md](../../ARCHITECTURE.md)（组件职责）、[CODE-STRUCTURE.md](../../CODE-STRUCTURE.md)（现状事实，刷新要求见 §4 B5）、[CODE-SPLIT-PRINCIPLE.md](../../principle/CODE-SPLIT-PRINCIPLE.md)（拆分判据唯一来源）、REFACTOR-SPLIT-PLAN（已归档，2026-11-07 执行计划）、[crate-architecture.md](../../store/crate-architecture.md)（crate 边界，不在本文范围） |
| 未完成项 | 移交 [code-structure-followups.md](../../store/code-structure-followups.md) |

> 本文回答四件事：代码放在哪（组织）、同级文件是否过多（扁平度）、中间层是否必要（间接层）、数据与逻辑是否混装。
> **「什么时候把一个文件拆开」的判据不在本文重述**，唯一来源是 [CODE-SPLIT-PRINCIPLE.md](../../principle/CODE-SPLIT-PRINCIPLE.md)；本文只补组织、扁平度、间接层与数据混装四类判据。
> 目录深度只作观察信号：路径段数不等于语义层数，段数不设上限（理由见 §2.2）。
> **§2.7 / §2.8 两张清单是拆分前的历史快照**，其中多数路径已在 B2 / B3 落地后改变，不得当作现状引用；未完成项与现存清单见 [code-structure-followups.md](../../store/code-structure-followups.md)。

---

## 0 与既有文档的关系

| 文档 | 关系 |
|---|---|
| `principle/CODE-SPLIT-PRINCIPLE.md` | 文件拆分、目录模块、依赖方向的唯一判据。本文所有条目都不得与之冲突 |
| REFACTOR-SPLIT-PLAN（已归档） | 2026-11-07 的执行计划。本文 §4 复核其全部未完成项并重新定级；两者冲突时以本文批次与目标结构为准，其 §2 已完成基线仍不得回退 |
| `CODE-STRUCTURE.md` | 只描述事实。本文 §2 的测量结果与它已有显著偏差（见 §2.10），落地后必须同步刷新 |
| `../../store/crate-architecture.md` | crate 划分与编译归属。本文**不增减 crate、不改依赖图** |

---

## 1 目标

| 编号 | 目标 | 判定方式 |
|---|---|---|
| S1 | 文件的职责数等于它需要响应的变化原因数 | 承接原则 §1；本文负责发现「长度未到 900 行但职责已混杂」的文件 |
| S2 | 每个非生成目录都对应一个可命名的变化原因（域 / 子域 / 职责），且不新增只做转发的中间层；组件自身的拆分目录（含 `index.tsx` 且只承载该组件 parts）是叶子容器，不是语义层。**深度段数只作观察信号，不作门槛**（依据见 §2.2） | 目录名与变化原因逐条对照；深度直方图仅记录 |
| S3 | 每个模块只有一个入口；不存在父子 barrel 重复同一导出清单；barrel 不含业务实现 | 依赖扫描 + 导出清单比对 |
| S4 | 静态数据与算法分离，单个逻辑文件不因数据表变长而变长 | 数据块占比 |
| S5 | `services/` 只放后端调用与传输，不放纯展示解析；`utils/` 不含业务语义 | import 图与调用面 |
| S6 | 单组件不同时承担「数据编排 + 表单/交互状态 + 布局」三类职责 | 组件内 hook 数、effect 数与 JSX 区块数 |
| S7 | 控制流嵌套 >4 层时抽返回值纯函数；JSX 嵌套 ≥13 层或组件 >400 非空行时按可见区块拆子组件 | 缩进深度直方图（仅作信号，不作门禁） |
| S8 | 拆分不改变对外行为、公开 API 与生成契约 | 契约零 diff + 测试通过 |
| S9 | 扁平目录按子域分组：直接子模块 ≥12 且存在 ≥2 个独立变化原因的子域（每个 ≥3 个文件）时建子目录；同一变化原因的 uniform peers 保持扁平 | 直接子文件计数 + 子域归类 |

---

## 2 现状测量（2026-11-16 快照）

### 2.1 规模

| 部分 | 文件数 | 非空行 |
|---|---|---|
| 前端 `src/`（不含测试与生成物） | 449 | 49,002 |
| 前端测试（`__tests__` / `*.test.*`） | 58 | 10,851 |
| Rust 子 crate 源码 | 322 | 55,237 |
| Rust 主 app 源码（`src-tauri/src` + `build.rs`） | 101 | 15,639 |
| Rust 测试（各 crate `tests/`） | 96 | 30,789 |

前端按顶层模块（不含测试与生成物）：

| 模块 | 文件 | 非空行 | 模块 | 文件 | 非空行 |
|---|---|---|---|---|---|
| `vault` | 76 | 9,793 | `mascot` | 26 | 2,061 |
| `chat` | 77 | 8,643 | `orb` | 10 | 1,654 |
| `tools` | 67 | 8,681 | `components` | 13 | 1,519 |
| `settings` | 74 | 7,336 | `features` | 9 | 738 |
| `history` | 18 | 2,625 | `utils` | 5 | 309 |
| `stores` | 19 | 2,383 | `audit` | 2 | 264 |
| `services` | 29 | 2,146 | `layouts` | 6 | 236 |

Rust 按包（`src/`，主 app 含 `build.rs`）：

| 包 | 文件 | 非空行 | 包 | 文件 | 非空行 |
|---|---|---|---|---|---|
| `assistant-tools` | 93 | 17,482 | `assistant-security` | 27 | 2,771 |
| `assistant-app`（主 app） | 101 | 15,639 | `assistant-sync` | 11 | 1,462 |
| `assistant-vault` | 62 | 9,808 | `assistant-crypto` | 6 | 1,071 |
| `assistant-providers` | 47 | 9,300 | `assistant-config` | 9 | 744 |
| `assistant-agent` | 39 | 7,911 | `assistant-orb` / `assistant-crash` / `assistant-i18n` | 4 / 2 / 1 | 382 / 146 / 124 |
| `assistant-storage` | 21 | 4,036 | | | |

数据来源：`node scripts/probe-structure.mjs`（`cargo metadata` + 文件系统遍历），每次结构改动后重跑，本文不再手工维护数字。

事实：`assistant-tools` 已是**源码量第一**的包（原 `crate-architecture.md` 记录的 702 行已失效），主 app 与 `assistant-vault` 次之。

### 2.2 目录深度（观察项）与扁平度

**深度是观察信号，不是门槛。** 原因：路径段数不等于语义层数。`vault/settings/sync/VaultSyncSettings/index.tsx` 是 4 段，但语义只有 3 层——`VaultSyncSettings/` 是该组件自己的 parts 桶（含 `index.tsx`，只承载该组件零件），不是「职责」层；`vault/core/contract/serde_json/JsonValue.ts` 的 4 段则来自生成物。反过来，`assistant-tools/src/os/tools/agent_grep/request.rs` 是 3 段且 3 层都真实存在（原则 §4 的 `foo/mod.rs` 形态）。用段数做门槛会把这两种情况判反。

更直接的证据：本文自身的 B3 + B4 计划按规则执行后必然产出 4 段——B3 把 `VaultSyncSettings.tsx` 拆成 `VaultSyncSettings/`（组件名目录），B4 又把这三项收进 `sync/`。一个从设计上就会被自身计划违反的门槛不能当判据。

实测直方图（`node scripts/probe-structure.mjs`，`src/` 之后的目录段数，已排除仅含生成物的目录）：

| 侧 | 深度分布 |
|---|---|
| 前端 | 1 段 20 个目录、2 段 69、3 段 38、4 段 3（另有仅含生成物的 `vault/core/contract`、`vault/core/contract/serde_json`，不计入） |
| Rust | crate `src/` 之后 1 段 39 个目录、2 段 24、3 段 8 |

前端那 3 处 4 段逐条说明（每一条都必须能被解释，解释不出的才算问题）：

| 路径 | 4 段来源 | 判定 |
|---|---|---|
| `vault/settings/sync/VaultSyncSettings/` | `sync/` 是子域（B4），`VaultSyncSettings/` 是组件 parts 桶（B3，含 `index.tsx`） | 合法：语义 3 层 |
| `vault/settings/sync/VaultConflictResolve/` | 同上 | 合法：语义 3 层 |
| `settings/shared/sections/PermissionAuditSection/` | `shared/sections/` 是既有父级，`PermissionAuditSection/` 是组件 parts 桶（B3，含 `index.tsx`） | 合法：语义 3 层 |

**扁平度**（单个目录的直接子文件数，含 `mod.rs` / `index.ts`，不含测试与生成物；这是真正作数的判据，见 R11）：

| 侧 | 直接子文件 ≥10 的目录 |
|---|---|
| 前端 | `chat/message` 19、`vault` 12（两处均达 R11 审查线，已按 uniform peers 判定维持扁平，理由见 §2.9） |
| Rust | `assistant-app/src/cmd` 18、`assistant-app/src` 13、`assistant-app/src/adapters` 12、`assistant-app/src/protocol/dto` 12、`assistant-vault/src/dto` 11、`assistant-agent/src` 11、`assistant-security/src/unlock_dialog/windows` 11、`assistant-app/src/cmd/vault` 10、`assistant-vault/src/security_db` 10、`assistant-storage/src` 10 |

结论：**深度不是当前的结构问题，扁平度才是**（B3/B4 落地后前端直接子文件 ≥12 的目录从 12 个降到 2 个，且这 2 个都已判定为 uniform peers）。判断分三种情况：

1. 若目录内文件全部响应同一类变化（`cmd` 的域入口、`adapters` 的端口实现、`protocol/dto` 的域 DTO、`unlock_dialog/windows` 的平台实现、各 crate `tests/`、23 个工具目录、`settings/core/types` 的类型文件），扁平是正确形态，不分组（R11）。
2. 若目录内可按**独立变化原因**归入 ≥2 个子域、且每个子域 ≥3 个文件，则判定为过扁，按子域建子目录（B4）。
3. `foo.rs` 与同名目录 `foo/` 并存是另一类问题，与扁平度无关（§2.4）。

深度不是问题，因此 B4 的分组**只增加语义子目录，不增加无意义的中间层**。分组与 B3 的组件目录叠加后会出现 4 段路径，这不需要处置——它不违反 R6（R6 判的是「有没有转发层」，不是段数），也不需要为了压回 3 段而撤销 `sync/` 这类真子域分组。

### 2.3 逻辑嵌套

| 侧 | 最深 | ≥10 层文件数 |
|---|---|---|
| Rust | 13 层：`assistant-providers/src/openai/responses_stream.rs`（743 行）；12 层：`anthropic/request.rs`（432）；11 层：`assistant-tools/src/os/tools/agent_ask_user.rs`（548，JSON schema 字面量）、`assistant-security/src/screen_lock/windows.rs`（284）；其余 5 个为 10 层（openai `message_converter` / `chat`、gemini `request` / `code_assist` / `message_converter/schema`） | 9 |
| 前端 | 16 层：`settings/provider/ConnectDialog/index.tsx`（226 行）；14 层：`chat/tool/ToolConfigPanel.tsx`（240）、`tools/text-diff/index.tsx`（160）；13 层：`components/commandPalette/CommandPalette.tsx`（286）、`tools/ui/ToolBox.tsx`（244）、`chat/message/MessageList.tsx`（241）、`settings/shared/sections/ToolAssemblySection.tsx`（209）等 | 36 |

口径与数据来源：`node scripts/probe-structure.mjs` 的「逻辑嵌套」章节（每文件取最大前导空白，Rust 除以 4、前端除以 2，跳过空行与 `//` 注释）。

说明：JSX 缩进天然较深，缩进本身不是缺陷判据；只有「≥13 层且单组件 >400 非空行」时，才视为需要按可见区块拆子组件的信号（S7）。Rust 侧深缩进集中在 `async_stream::stream!` 的 `match` 内，处理方式按原则 §5「宏边界不可搬移」执行。

### 2.4 间接层（入口与重复导出）

| 现象 | 位置 | 事实 |
|---|---|---|
| 死 barrel | `src/mascot/core/index.ts`、`src/settings/core/types/index.ts` | 全仓无任何 import 解析到这两个文件（消费方都直接引用子模块） |
| 重复导出清单 | `src/mascot/index.ts` 与 `src/mascot/core/index.ts` | `core/index.ts` 的全部导出在 `mascot/index.ts` 中再列一遍，改一处需同步两处 |
| 文件与同名目录并存 | `assistant-agent/src/kernel/ports.rs` + `ports/test_doubles.rs`；`assistant-tools/src/os/agent_read.rs` + `agent_read/window.rs`；`assistant-tools/src/sandbox/windows.rs` + `windows/acl.rs`（2026-11-16 已改为 `windows/mod.rs`，对外路径 `sandbox::windows` 不变） | 与原则 §4「`foo.rs` → `foo/mod.rs`」不一致，且让模块深度多算一层 |

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
| `assistant-tools/src/gateway.rs`（1170） | 审批域类型 / 网关决策 / 审批上下文构建 / 授权与审计存储 / 审批键归一化 / 测试替身 | `gateway/{mod,approval_key,context,decision,stores,testing}.rs` |
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

| 项 | 文档原值 | 实测（2026-11-16） |
|---|---|---|
| `assistant-tools` | 702 行 / 8 文件 | 17,482 非空行 / 93 文件（另含 `mcp/`、`compact.rs`、`policy.rs`、`runtime/pipeline.rs` 等文档未列模块） |
| `assistant-agent` | 2,192 行 / 13 文件 | 7,911 非空行 / 39 文件（含 `kernel/`、`agent/`） |
| `assistant-vault` | 9,143 行 / 47 文件 | 9,808 非空行 / 62 文件 |
| 主 app | 6,702 行 / 39 文件 | 15,639 非空行 / 101 文件 |
| Rust 合计 | 37,903 行 | 源码 70,876 非空行（不含测试） |

这些偏差会让后续结构判断建立在错误的规模认知上，属本规格的附带交付（§4 B5）。B5-1 已按上表刷新 `CODE-STRUCTURE.md`，上表保留为刷新前快照；B5-2 待做。

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
| R6 | **无转发层**（原「深度上限」已降级）：新增目录必须对应可命名的变化原因（子域或职责）；不得新增只做转发的中间目录；同层不得出现两个仅靠路径区分、职责相同的目录。**路径段数本身不是判据**——一个路径段不等于一个语义层（组件 parts 桶、生成物目录都会占段而不占层） | 目录名与变化原因对照；转发层用 import 图判定（该目录是否只有 re-export） |
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

> 前置下沉已完成（2026-11-16）：`os/scope.rs`、`os/walk.rs`、`os/fs_util.rs` 三个 `pub(crate)` 叶子落地，`agent_grep` / `agent_glob` / `agent_move` / `agent_delete` 的本地副本删除并改为调用共享叶子；对外 API 与契约不变，`cargo check` / `cargo clippy` / 全量单测通过。
>
> 已完成（2026-11-16）：`protocol/dto/chat.rs`（810）→ `dto/chat/{conversation,message,usage,event,content}.rs` + `chat/mod.rs` 重导出，对外路径 `dto::*` 不变；`tests/protocol_bindings.rs` 的 DTO 源码扫描改为递归子目录（原为单层 `dto/*.rs`），生成物 `protocol.contract.ts` 无 diff。
>
> 已完成（2026-11-16，`assistant-tools`）：`gateway.rs`（1252）→ `gateway/{mod,approval_key,context,decision,stores,testing}.rs`（`mod.rs` 保留 `ToolGateway` 与全部 `pub use`，对外导出集合不变）；`os/path.rs`（1065）→ `os/path/{mod,args,membership,protected,resolve}.rs`（组件前缀与同路径判定留在 `mod.rs`，`resolution_base` 以 `pub(crate)` 重导出）；`os/agent_delete.rs`（946）→ `os/agent_delete/{mod,attrs,exec,scan,target,trash}.rs`。三项均保持 `cargo check` / `cargo clippy` / 单测与集成测试通过。
>
> 已完成（2026-11-16，`assistant-tools` P1 收尾）：`mcp/stdio.rs`（979）→ `mcp/stdio/{mod,client,transport,codec,stderr}.rs`（`mod.rs` 保留三个公开常量与 `MAX_TOOL_PAGES`，`McpClient` / `RpcFailure` / `spawn_stderr_logger` 原样重导出）；`compact.rs`（915）→ `compact/{mod,types,head,anchor,truncation,preview}.rs`（`build_compact` / `rebuild_compact` / `error_kind_from_text` 与取值辅助函数留在 `mod.rs`，公开常量移入 `types.rs` 后重导出）。`cargo check` / `cargo clippy` / `cargo fmt --check` / 全量单测（212 + 16 组集成测试）全部通过。

> 已完成（2026-11-16，`assistant-app`）：`cmd/conversation.rs`（841）→ `cmd/conversation/{mod,list,detail,detail_dto,mutate,branch,export}.rs`（列表/搜索、详情与分页、DTO 组装、变更与批量删除、分支与分叉、导出）。`mod.rs` 只保留 `emit_list_changed` 与 glob 重导出：`#[command]` 生成的 `__cmd__*` 宏随 `pub use x::*` 一起导出，逐项 `pub use x::{f}` 只导出函数、会让 `ipc.rs` 的 `generate_handler!` 找不到宏，因此必须保持 glob。对外命令名与签名全部不变，`cargo check` / `cargo clippy` / `cargo fmt --check` / 全量单测通过。

> 已完成（2026-11-16，`assistant-app`）：`cmd/permission.rs`（721）→ `cmd/permission/{mod,grants,audit}.rs`（授权 CRUD 与 DTO 映射、审计写入词表、审计查询与分页夹取）。`tier_label` / `short_hash` 两处列表与审计共用的取值辅助下沉到 `mod.rs`；`audit_resource_from_key` / `write_grant_audit` 与 `GRANT_ACTION_*` 词表留在 `audit.rs` 以 `pub(super)` 供 `grants.rs` 复用，脱敏摘要与展示摘要仍是两份独立函数（不合并）。对外命令名与签名不变，`cargo check` / `cargo clippy` / `cargo fmt --check` / 全量单测通过。

> 已完成（2026-11-16，`assistant-vault`）：`security_db/mod.rs`（740）→ `security_db/{mod,state,session,anchor}.rs`，连接与 pragma 归入既有 `schema.rs`。`mod.rs` 只留模块声明、重导出与跨子模块共享的常量（两个 pragma 串、`BACKFILL_EVENTS` / `J_CHANNEL_EVENTS`、备份前后缀与 `MAX_BACKUPS`、`JSONL_SYNC_KEY`）；`session.rs` 承载 `VaultSession` 与两个 DEK/连接守卫，`anchor.rs` 承载 `AnchorExpectation`，`state.rs` 承载 `SecurityDbState` 与其全部方法。`rekey.rs` 直接读 `self.session`、调用 `backup_dir_for` / `prune_backups`，故 `session` 字段与这两个辅助函数改为 `pub(super)`；`open_connection` / `probe_read` 由 `schema.rs` 定义并在 `mod.rs` 以 `pub(crate) use` 重导出，外部路径不变。对外 API 与契约不变，`cargo check` / `cargo clippy` / `cargo fmt --check` / 全量单测（含 vault 15 组集成测试）通过。

> 已完成（2026-11-16，`assistant-tools` P2 首批）：`os/agent_grep.rs`（752）→ `os/agent_grep/{mod,request,outcome,search,sink,probe,render}.rs`（`mod.rs` 保留 `AgentGrepTool`、`TOOL_ID` 与全部 `pub const`，`GrepRequest` 解析与 `build_include` 移入 `request.rs`，`SearchOutcome` / `NonUtf8Probe` 移入 `outcome.rs`，遍历与文件判定移入 `search.rs`，`FileSink` 移入 `sink.rs`，零命中探测移入 `probe.rs`，footer 与结果渲染移入 `render.rs`）；`os/agent_write.rs`（523）→ `os/agent_write/{mod,encoding,atomic,lines,preview}.rs`（`mod.rs` 保留 `AgentWriteTool`、`WriteRequest`、两个公开常量与 `too_large_message`，编码解析与 payload 组装移入 `encoding.rs`，原子写与现有内容读取移入 `atomic.rs`，行数统计移入 `lines.rs`，审批预演移入 `preview.rs`）。两者对外导出集合不变，`cargo check` / `cargo clippy` / `cargo fmt --check` / 全量单测（212 + 16 组集成测试）通过。

> 已完成（2026-11-16，`assistant-tools` P2 续）：`mcp/manager.rs`（781）→ `mcp/manager/{mod,types,lifecycle}.rs`。`mod.rs` 只保留 `McpServerManager` 结构、`new` / `with_backoff_base` / `snapshot` / `snapshots` / `with_entry` 与 `pub use types::*`，以及全部 8 项状态机单测；`types.rs` 承载状态机定义（`McpServerStatus` 与 `allows` 迁移表）、`backoff_delay` 与退避常量、`McpServerConfig` / `McpConnection` / `McpTransport` / `ServerCaller` / `McpServerSnapshot` / `ServerEntry` 与 `transition`；`lifecycle.rs` 承载 `enable` / `disable` / `mark_config_error` / `forget` / `reconnect` / `start` / `stop` / `mark_error` / `sync_tools`。`ServerEntry` 字段与 `DEFAULT_BACKOFF_BASE` 改 `pub(super)` 供 `lifecycle.rs` 跨子模块访问，对外导出集合不变，`cargo check` / `cargo clippy` / `cargo fmt --check` / 全量单测（212 + 16 组集成测试）通过。

> 已完成（2026-11-16，`assistant-tools` P2 续）：`mcp/bridge.rs`（731）→ `mcp/bridge/{mod,naming,defs,call}.rs`。`naming.rs` 承载 `TOOL_PREFIX` / `McpToolName` / `normalize_segment` / `build_tool_names` / `short_hash`；`defs.rs` 承载 `McpAnnotations` / `McpToolDef` / `McpTool` 与 `impl Tool`、`build_tools`；`call.rs` 承载 `McpCallOutcome` / `McpCaller` / `guarded_call` / `map_outcome` / `map_content`。`mod.rs` 只留模块声明、`pub use` 与全部 26 项单测（跨命名、安全标记推断与结果映射三层，测试辅助桩 `StubCaller` / `def` / `tool` 不再重复）；`McpTool.description` 与 `short_hash` 改 `pub(super)` 供测试与同级子模块访问，对外导出集合不变，`cargo check` / `cargo clippy` / `cargo fmt --check` / 全量单测通过。

> 已完成（2026-11-16，`assistant-vault` P2 续）：`vault_file/mod.rs`（714）→ `vault_file/{mod,entry,meta}.rs`。`mod.rs` 只保留常量、`CipherProvider`、`VaultFileManager` 结构与其仓库布局/路径方法（`new` / `cipher_algorithm` / `vault_dir` / `entries_dir` / `ensure_structure` / `ensure_gitignore` / `clean_temp_files` / `check_repository_layout` / `entry_file_path`）以及私有的 `write_atomically`；`entry.rs` 承载条目文件读写（`export_entry` / `import_entry` / `decode_entry_file` / `reencrypt_entries` / `read_all_entries` / `remove_entry_file` / `list_entry_files`、`decode_entry_content` / `peek_entry_dek_version` / `ImportedEntry::into_import`）；`meta.rs` 承载 `.vault-meta` 与清单/完整性（`meta_path` / `read_vault_meta_v2` / `write_vault_meta_v2` / `compute_manifest` / `meta_file_hash` / `verify_manifest` / `verify_meta_integrity` / `canonical_meta_bytes` 及全部校验辅助函数）。三个公开自由函数经 `mod.rs` 的 `pub use` 重导出，外部路径 `crate::vault_file::*` 不变；`tests/legacy_residue.rs` 的 v1 残留白名单随符号迁移同步指向 `vault_file/meta.rs`。`cargo check` / `cargo clippy` / `cargo fmt --check` / 全量单测通过。

> 已完成（2026-11-16，`assistant-vault` P2 续）：`dto/convert.rs`（472）→ `dto/convert/{mod,sync,error,misc,entry}.rs`。`sync.rs` 承载全部同步域 DTO 转换（仓库状态、就绪度、远端状态、待删除条目、不可读原因、`SyncResult`、冲突文件/预览、`SyncConfig` 双向、`HistoryCommit`、`EntryPreview` 系列）；`error.rs` 承载 `VaultError` / `SyncError` → `VaultErrorDto`；`misc.rs` 承载 TOTP 算法、`LockReason`、`otpauth` 解析与 HIBP 条目；`entry.rs` 承载 `VaultEntrySnapshot` → `VaultEntryDetail` / `VaultEntrySummary`。四个子模块只含 `impl` 块、不导出新符号，`dto/mod.rs` 的 `mod convert;` 与对外路径不变；`cargo check` / `cargo clippy` / `cargo fmt --check` / 全量单测通过。

> 已完成（2026-11-16，`assistant-app` P2 续）：`adapters/question.rs`（580）→ `adapters/question/{mod,registry,prompter,dto}.rs`。`registry.rs` 承载 `PendingQuestionRegistry`、`PendingQuestion` 与答案语义校验 `validate_answer_semantics`，并保留全部 15 项登记表单测；`prompter.rs` 承载 `TauriQuestionPrompter`、`PendingQuestionGuard` 与 `QuestionPrompter::ask` 的 fail-closed 分支；`dto.rs` 承载 `ASK_USER_TOOL_NAME` 常量与 `question_dto` 映射及其单测。`register` / `discard` 改 `pub(super)` 供 `prompter.rs` 跨子模块访问，`mod.rs` 以 `pub use` 重导出 `PendingQuestionRegistry` / `TauriQuestionPrompter`，`adapters/mod.rs` 的对外路径不变；`cargo check` / `cargo clippy` / `cargo fmt --check` / 全量单测通过。

> 已完成（2026-11-16，`assistant-sync` P2 收尾）：`git_sync/git.rs`（531）→ `git_sync/git/{mod,repo,branch,remote,log,plumbing}.rs`。`plumbing.rs` 承载命令构造与输出断言（`cmd_git` / `cmd_assert_status` / `git_output` / `git_stdout_ok` / `git_stdout_ok_or`）、`BIN_NAME`、`sanitize_git_stderr` 与 `git` 主入口；`repo.rs` 承载仓库初始化/克隆、暂存提交、状态与冲突处理、`git_state` / `git_abort`、`git_rev_list_left_right_count`；`branch.rs` 承载当前分支、上游与 `branch.<name>.remote` 配置、引用哈希；`remote.rs` 承载 `push` / `pull` / `fetch`、远端增删查与 `git_ls_remote`；`log.rs` 承载 `git_last_pull_time`、`git_last_commit_info` 与 `RawLogEntry` / `git_log`。底层封装在 `plumbing.rs` 内以 `pub(super)` 暴露、`mod.rs` 只以 `pub(crate) use plumbing::git` 回挂主入口，其余 `pub use` 保持 `assistant_sync::git_sync::git::*` 对外导出集合完全不变；`cargo check` / `cargo clippy` / `cargo fmt --check` / 全量单测（含 `git_sync` 19 项集成测试）通过。

| 优先级 | 条目 | 依赖 |
|---|---|---|
| P1 | `gateway.rs`、`os/path.rs`、`os/agent_delete.rs`、`compact.rs`、`mcp/stdio.rs` | 无；纯搬移为主 |
| P1 | `protocol/dto/chat.rs` | 无 |
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

**P1 拆分前手测清单**（拆分后逐条回归，确保行为零变更）：

- `PermissionAuditSection`：范围切换与 limit 切换均重置分页并重新拉取；翻页窗口上界冻结在首页首行 `atMillis`；刷新按钮重置分页 + 重新拉取并显示 loading；上一页/下一页受 `hasMore` 约束；列宽拖拽（pointerdown/move/up）实时改宽、`body` 光标 `col-resize` 且 `userSelect: none`、结束后恢复；列宽双击恢复默认；统计条决策数/拒绝率/批准率（分母 0 显示 `—`）；turn 列标题与 iteration 拼接、未归属提示；加载态 / 空态 / 错误态；定高滚动区与表头吸顶。
- `VaultSyncSettings`：就绪度全部分支 alert（unknown / failed / repoState / noSync / noRemote / dirty / ahead / behind / diverged / inSync / readyUnchecked）；`repoState` 分支 abort 确认后提示成功；`noRemote` CTA 聚焦远端 URL 输入框；测试连接成功 toast 显示分支数；SSH hosts 保存时 split / trim / lowercase 并 patch `vault` 配置（Windows 隐藏该输入）；`noSync` 下初始化 / 克隆（本地已有条目时克隆禁用并提示）；非 `noSync` 下立即同步 / pull / push（`pendingRemovals` 非空时 push 禁用）；待删除条目 banner 的全量 / 部分标题与确认回调；不可读条目与 `keyMismatch` 告警的导出入口跳 Recovery；error alert 关闭时同时清理 `error` 与 `syncError`；上次同步相对时间与 Danger Zone 入口。
- `VaultConflictResolve`：打开时重置选择与遮蔽状态并拉取 preview；preview 加载中 / 失败 / 成功三态；对比表「仅本地 / 仅远程 / 已变更」标签与未变更行隐藏、勾选后显示未变更并提示隐藏条数；敏感字段点击显示后按 `expiresAt` 自动重新遮蔽、关闭弹窗清理计时器；`useLocal` / `useRemote` 走确认弹窗、`mergeBoth` / `skip` 直接提交且 `mergeBoth` 传 `remote_copy_suffix`；提交成功 toast 后 `onClose`；local / remote 任一 preview 缺失时回退为双卡片视图。

> 已完成（2026-11-16，前端 P1 全批）：`PermissionAuditSection.tsx`（666）→ `PermissionAuditSection/{index,range,columns,format,stats,useColumnResize,AuditTable}.ts(x)`（范围常量与 `rangeToSince`、列键与宽度夹取、时间与资源摘要格式化、统计口径、pointer 拖拽生命周期与 `body` 光标/选中锁定、定高滚动表格各自独立）；`VaultSyncSettings.tsx`（652）→ `VaultSyncSettings/{index,syncResult,statusAlert,SyncNotices,RemoteConfigForm,SyncActions}.ts(x)`（错误文案映射与同步结果/不可读条目派生、就绪度→告警装配、四类提示、远端与 SSH 表单、同步动作与 Danger Zone 入口）；`VaultConflictResolve.tsx`（635）→ `VaultConflictResolve/{index,types,useRevealSecrets,secretCells,compareRows,VersionCard,ConflictCompare}.ts(x)`（对比行类型与时间格式化、遮蔽值与自动重遮蔽计时、遮蔽单元格与标签渲染、字段摊平、单侧版本卡片、对比表与未变更折叠）。三者对外默认导出与导入路径不变，交互与行为零变更，`npm run typecheck` / `npm run lint` / `npx vitest run src/vault` 通过。2026-11-16 复核：`PermissionAuditSection/` 保留在 `settings/shared/sections/`（同级 `sections` 全部是单页消费者，`shared/` 的现状命名与它们一致；搬出只服务于压段数，故撤销）。

**P2 关键交互基线**（拆分后逐条回归）：

- `AgentModeSettings`：搜索框同时过滤内置/自定义两段；新增与编辑共用同一抽屉（编辑内置项时 `kind` 与 `name` 禁用，自定义项可删）；`reasoningEnabled` 关闭时 `reasoningEffort` 归零；`kind === "agent"` 才展示工具装配区，切回 `chat` 时保存沿用编辑态工具配置；内置项重置 / 自定义项删除均走确认弹窗；保存成功 toast 文案按新增/编辑区分；头像预览随 `kind` 回退到 agent/chat 默认；页面底部权限授权与权限审计两段常驻。
- `MessageList`：进入会话按位置记忆落位（记住 pinned 与否）；流式中贴底自动跟随、用户上滑后停止跟随并累计未读数；停止流式且回到贴底阈值内自动恢复跟随并清零未读；触顶自动前插分页只触发一次且补偿后保持视口；「加载更早」按钮走同一补偿入口；程序性位移保护期内的滚动事件被吞掉；`ResizeObserver` 不可用时按渲染高度降级跟随；切换会话不写旧会话位置、卸载时保存最后位置；空消息时仍渲染滚动容器。
- `GhostyMascot`：17 种表情预设的眼睛缩放/眉毛倾角/腮红浓度；`speaking` 走 viseme 嘴形、其余走固定嘴形；`waving` / `celebrating` / `dancing` 显示手臂并摆动；`thinking` 眨眼周期 4200ms、其余 2600ms，眨眼期间高光隐藏；浮动、光点挤压与地面缩放同步；`animated: false` 时不注册动画帧；`flat` 变体下暗部透明度降级。
- `ConnectDialog`：编辑态隐藏类型字段并锁定格式；预设与自定义格式的标题/描述文案分支；`local` 格式隐藏 API Key 且不校验必填；新增态 API Key 与地址必填、类型仅在自定义时必填；编辑态空密钥按 `keepIfEmpty` 保留；「获取模型」在新增态缺密钥时给出警告、编辑态无密钥走 `listModels`、其余走 `previewModels`；获取成功后回填默认模型选项并 toast 数量；模型清单最多展示 20 条且点击回填 `defaultModel`，超出部分提示剩余条数；打开时按编辑/新增分别回填。

> 已完成（2026-11-16，前端 P2 全批）：`AgentModeSettings.tsx`（约 560）→ `pages/agentMode/{index,avatar,fieldSchemas,useModeForm}.ts(x)`（头像预览与选项、字段定义与推理档位、抽屉生命周期与保存/重置/删除、列表与搜索装配各自独立）；`MessageList.tsx`（约 505）→ 保留装配 + 新增 `useMessageScroll.ts`（落位、跟随、前插补偿、未读计数与位置记忆整体下沉，源码门禁按原路径仍生效）；`GhostyMascot.tsx`（约 494）→ `svg/{facePresets,mouthPaths,useGhostyAnimation}.ts(x)`（表情预设表、固定嘴形、逐帧动画写入；2026-11-16 由 `svg/ghosty/` 平铺回 `svg/`，`ghosty/` 无 `index.tsx`、只是上一级组件的 parts 容器，属无独立变化原因的零件桶（违反 R6）故撤）；`ConnectDialog.tsx`（约 397）→ `ConnectDialog/{index,fieldSchemas,useConnectForm,ModelPreviewList}.ts(x)`（字段与获取模型动作、表单状态与提交装配、模型清单预览、弹窗装配）。对外默认导出与导入路径不变（`ConnectDialog` 目录名与导入路径一致），交互与行为零变更，`npm run typecheck` / `npm run lint` / `npx vitest run src/mascot src/settings src/chat`（366 项）通过。

> 已完成（2026-11-16，前端 P3）：`turnOrchestrator.ts`（约 520）→ 仅抽 `stream/phase.ts`（`ChatPhase` 与 `TurnPhase`/`SessionState` → UI phase 映射）与 `stream/title.ts`（`fallbackTitleFromUserMessage`），`turnOrchestrator.ts` 以 `export type` / `export { ... } from` 保持原有导出面，编排骨架与策略分派未动。`npx vitest run src/services`（67 项）通过。

### B4 扁平目录分组

前置：B0–B3 完成后执行，避免与文件拆分并发修改同一批文件。全批为**纯搬移**：子目录名对应稳定子域，import 同步更新，不留兼容 shim，不改行为。

分组判据见 R11：直接子模块 ≥12 时审查；只有子域对应独立变化原因、且每个子域 ≥3 个文件才分组。

| 优先级 | 目录（直接文件数） | 子域与目标 | 备注 |
|---|---|---|---|
| P1 | `src/vault/settings`（15） | `sync/`（3：VaultSyncSettings / VaultConflictResolve / SshAgentTab）、`recovery/`（5：RecoveryKeyDialog / RecoveryKeyForm / RecoveryTab / RotateDekDialog / printPaperKey）、`security/`（5：ChangePasswordTab / SecurityLogTab / DangerZone / DangerZoneConfirm / DangerZoneTab）；`VaultSettings`、`ImportExportTab` 留根层 | 先完成 B3 对 VaultSyncSettings / VaultConflictResolve 的拆分，再搬移；2026-11-16 复核保留 `sync/`：三项同属「远端同步」一个变化原因，与 `recovery/`、`security/` 对称并对应 `VaultSettings` 的 tab 分组，不因拆分后直接子文件降至 3 而撤销 |
| P1 | `src/tools`（14） | `framework/`（6：registry / types / categories / capabilities / os / miniNavigate）、`ui/`（8：ToolBox / MiniToolBox / ToolPage / ToolCard / ToolSearchBar / FavoriteButton / CollapsibleToolMenu / ToolCapabilityWrapper）；23 个工具目录不动 | `registry.ts` 内 23 条 `import("./<tool>")` 需改为 `../<tool>`，导入面集中在 `tools/` 与少量设置页 |
| P1 | `src/chat/core`（17） | `stream/`（3：streamCoalescer / useTypewriter / optimisticMessage）、`message/`（4：messageTree / contentBlockTypes / contentBlockUtils / conversationLink，纯逻辑消息模型，区别于 UI 层 `chat/message/`）；其余 10 个文件留根层 | 其余子域各 2 个文件，未达 3 个，不再细分 |
| P2 | `src/services`（24） | `chat/`（3）、`tool/`（3：toolService / permissionService / mcpService）、`system/`（5：tauriInvoke / crossWindowEvent / logger / globalErrorHandler / reactProductionError）、`desktop/`（4：storageService / workspaceService / orbService / windowService）；`history` / `settings` / `model` 各 2 个留根层 | B1 移出 `audit*` 后剩 22 个；分组后根层约 7 个单文件 |
| P2 | `src/stores`（18） | `chat/`（6：chatStore / chatConversationSlice / chatMessageSlice / chatCompactSlice / chatStorePorts / chatStreamStore）、`system/`（4：themeStore / localeStore / notifyStore / windowModeStore）；`agentModeStore` 并入已有 `agent/` | 其余 7 个领域 store 留根层 |
| P2 | `assistant-tools/src/os`（14 + 3 目录） | `tools/`（9 个工具文件 + `agent_edit/` / `agent_read/` / `fs_link/`）、`shared/`（path / output / text_encoding / process_output） | 触碰 17 个文件的 26 处 `crate::os::*` 引用；`os/mod.rs` 保留 `pub use` |
| P2 | `src/settings/core`（13） | `registry/`（5：builtinRegistry / definePage / initSettingSystem / pageRegistry / sideEffectRegistry）、`runtime/`（8：confRuntime / snapshot / settingsDispatcher / optimisticUpdate / query / insertUnique / useConfGroup / useConfPatchHandlers）；`types/` 已存在 | 导入面集中在 `settings/` 与 `features/` |
| P3 | `src/chat/message`（19） | 暂不分组：19 个文件同属「消息行渲染」一个变化原因；仅当 `input/` 或 `scroll/` 出现独立变化时再建子目录 | 观察项，触发条件见 R11 |
| P3 | `src/vault/core`（13） | `hooks/`（4：useVaultAutoLock / useVaultEvents / useVaultStatusReady / useVaultUnlock）、`display/`（6：paperKey / syncDisplay / vaultError / vaultErrorText / vaultColors / vaultCategory）；`vaultStore` / `vault.service` / `vault.types` 留根层 | 与 `slices/`、`contract/` 同级，不新增转发层 |
| P3 | `src/vault`（12） | 暂不分组：列表 / 外壳 / 抽屉同属「保险库页面装配」一个变化原因 | 页面装配出现独立子变化时再评估 |
| — | `src/chat/tool`（11）、`src/orb`（10） | 未达 12 的审查阈值，不分组 | — |
| P3 | `assistant-storage/src`（13） | `tools/`（3：tool_history / tool_favorites / mcp_servers）；`db` / `migrations` / `attachment_store` / `content_codec` 等各 1–2 个文件留根层，不建目录 | 触碰各 crate 调用点，收益中等 |
| P3 | `assistant-agent/src/kernel`（13） | `turn/`（runner / phase / state / outcome / emit / persist / stream）；`ports/`、`request`、`tasks`、`limits`、`ids` 留根层 | 与 `kernel` 公开路径同步 |
| P3 | `assistant-tools/src`（13） | `contract/`（trait / definition / context / result）、`runtime/`（gateway / policy / exec / pipeline / registry / compact / question / storage） | `lib.rs` 的 `pub use` 路径保持不变 |
| P3 | `src-tauri/src/cmd`（19） | 暂不分组：多数文件本身即域入口，分组需同步 `cmd/mod.rs` 与 `ipc.rs`，改动面大于收益 | 观察项 |

**保持扁平（明确不动）**：`src-tauri/src` 根的 13 个装配入口、`adapters/` 13 个端口实现、`protocol/dto/` 12 个域 DTO、`assistant-vault/src/dto/` 12 个域 DTO、`unlock_dialog/windows/` 11 个平台实现、各 crate `tests/`、23 个 `src/tools/*` 工具目录、`src/settings/core/types/` 的类型文件。这些目录的文件是 uniform peers：同一变化原因、互相平级、没有可命名的子域。

`os/` 分组是原 B4 的正式化：不再按「模块数 ≥20」延后，而是作为 P2 执行；若在 B2 拆分 `os/agent_*` 文件时已建立子目录，则在同一 PR 内顺带完成 `shared/` 下沉，避免二次搬移。

> 已完成（2026-11-16，B4 全批）：P1 三项 —— `src/vault/settings` → `sync/`（3）、`recovery/`（5）、`security/`（5），`VaultSettings` / `ImportExportTab` 留根层；`src/tools` → `framework/`（6）、`ui/`（8），23 个工具目录不动（`registry.ts` 的 `import("./<tool>")` 同步改为 `../<tool>`）；`src/chat/core` → `stream/`（3）、`message/`（4），其余 10 个文件留根层。P2 四项 —— `src/services` → `chat/`、`tool/`、`system/`、`desktop/`（根层剩 7 个单文件）；`src/stores` → `chat/`（6）、`system/`（4），`agentModeStore` 并入已有 `agent/`（根层剩 7 个领域 store）；`assistant-tools/src/os` → `tools/`（9 个工具文件 + `agent_edit/` / `agent_read/` / `fs_link/`）、`shared/`（path / output / text_encoding / process_output / fs_util / scope / walk / test_support），`os/mod.rs` 保留原 `pub use` 导出面；`src/settings/core` → `registry/`（5）、`runtime/`（8），`types/` / `field/` / `agentKind/` / `provider/` 不动。P3 四项 —— `src/vault/core` → `hooks/`（4）、`display/`（6），`vaultStore` / `vault.service` / `vault.types` 留根层；`assistant-storage/src` → `tools/`（mcp_servers / tool_favorites / tool_history），`lib.rs` 以 `pub use tools::{...}` 保持 `assistant_storage::tool_history` 等外部路径；`assistant-agent/src/kernel` → `turn/`（runner / phase / state / outcome / emit / persist / stream），`ports/` / `request` / `tasks` / `limits` / `ids` 留根层；`assistant-tools/src` → `contract/`（trait / definition / context / result）、`runtime/`（gateway / policy / exec / pipeline / registry / compact / question / storage），`lib.rs` 的 `pub use` 路径保持不变。
>
> 暂不分组项复核结论不变：`src/chat/message`（19，同一「消息行渲染」变化原因）、`src/vault`（12，同一「保险库页面装配」变化原因）、`src-tauri/src/cmd`（18，多数文件即域入口，改动面大于收益）维持扁平；`src/chat/tool`（11）、`src/orb`（10）未达 12 的审查阈值。以上数字来自 2026-11-16 的 `probe-structure.mjs` 实测。
>
> 源码门禁随搬移同步（行为不变、仅路径）：`assistant-tools/tests/pipeline.rs` 的 `include_str!("../src/pipeline.rs")` → `../src/runtime/pipeline.rs`；`assistant-vault/tests/dto_contract.rs` 的 `vaultError.ts` 读取路径 → `src/vault/core/display/vaultError.ts`。另清理 `assistant-i18n` 的未使用 `serde` 依赖（仅 `serde_json` 在用），消除 `cargo check` 的 unused dependency 警告。
>
> 验证：`npm run typecheck` / `npm run lint` / `npx vitest run`（58 个测试文件）与 `cargo check --workspace --all-targets` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo test --workspace` 全部通过，零错误零警告；对外 API 与运行时行为零变更。

### B5 文档刷新

| 条目 | 动作 | 状态 |
|---|---|---|
| B5-1 | 更新 `CODE-STRUCTURE.md` 的模块清单（补 `mcp/`、`kernel/`、`agent/`、`sandbox/`）、脚本清单、测试布局与定位索引 | ✅ 2026-11-15 |
| B5-2 | 更新 `crate-architecture.md` 各 crate 行数/文件数与模块列表 | 待做 |
| B5-3 | REFACTOR-SPLIT-PLAN（已归档），不再需要顶部标注 | ✅ 2026-11-15（作废） |

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

1. S2 / R6：重跑目录统计，**逐条解释**每个 ≥4 段目录的成因（子域？组件 parts 桶？生成物？）。解释不出的即失败，典型形态是只做转发的中间层、无 `index.tsx` 的零件桶、只装 1 个文件的空壳目录。段数本身不设上限，第 5 段、第 6 段同理按成因判定。
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

本文全部数字的唯一来源是仓库内脚本，不手工统计：

```powershell
# 生成完整报告（Markdown）
node scripts/probe-structure.mjs --limit 40 --out temp-backup/structure-probe.md

# 生成机读快照（JSON，便于逐项核对）
node scripts/probe-structure.mjs --json --out temp-backup/structure-probe.json
```

脚本输出章节与本文对应关系：

| 脚本章节 | 对应本文 | 口径 |
|---|---|---|
| 规模（Rust 各 crate / 前端） | §2.1 | 非空行；前端区分「非测试非生成」与测试 |
| 顶层模块（非测试非生成） | §2.1 前端表 | 按 `src/` 之后第一段目录聚合 |
| 目录深度直方图 | §2.2 | `src/` 之后目录段数；仅含生成物的目录单列 |
| 深度观察（≥4 段） | §2.2 / §5 验收第 1 条 | 逐条解释成因，不作失败判定 |
| 扁平度（直接子 `.rs` ≥10 / 直接子文件 ≥12） | §2.2 / R11 | 含 `mod.rs`、`index.ts`；不含测试与生成物 |
| 逻辑嵌套 | §2.3 | 最大前导空白，Rust `/4`、前端 `/2`，跳过空行与 `//` |

改了统计口径就改脚本，本文只引用脚本输出，不再维护 PowerShell 片段（避免文档与脚本两套口径再次漂移）。

---

## 9 定位索引

| 需求 | 位置 |
|---|---|
| 新增前端领域模块 | 顶层 `src/<domain>/`，入口只放 barrel；实现按 `core/`（纯逻辑）、UI 目录、`slices/`（状态）分层 |
| 新增前端跨域展示逻辑 | 独立领域目录（如 `src/audit/`），禁止放 `services/` 或让一方领域依赖另一方 |
| 新增静态大表 | `<tool>/<name>.data.ts`，与算法文件分离 |
| 新增 Rust 子模块 | `foo.rs` → `foo/mod.rs`；共享叶子下沉到被依赖方都不依赖的独立模块 |
| 判断能否新增一层目录 | 问「这层对应哪个可命名的变化原因」：答得出（子域 / 职责）就建，答不出（只转发、只是零件桶、只装 1 个文件）就不建；不看段数（R6） |
| 某目录同级文件增多 | 先按 R11 归类：≥2 个独立变化原因的子域且各 ≥3 个文件才建子目录；同一变化原因的 uniform peers 保持扁平 |
| 新增工具实现 | `assistant-tools/src/os/<tool>.rs`；出现第二个变化原因时再升级为目录模块 |
| 判断是否该拆 | `../../principle/CODE-SPLIT-PRINCIPLE.md` |
| 判断放哪个 crate | `../../store/crate-architecture.md` §5 |
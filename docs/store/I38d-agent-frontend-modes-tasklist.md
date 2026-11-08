# I38d — P6 前端集成与 Agent 模式任务清单

> 规格：[I38d-agent-frontend-modes-spec.md](./I38d-agent-frontend-modes-spec.md) §1–§4、§6 验收标准
> 阶段：[I38-agent-roadmap.md](./I38-agent-roadmap.md) P6
> 顺序：C1 → C1a → C1b → C1c → C1d → C2 → C3 → C4 → C5 → C6 → C7 → C8
> 勾选条件：代码已改 **且** 对应验收已跑。
> **状态唯一位置**：本清单是 P6 任务级进度的唯一来源；规格只定义「做什么」与「验收什么」，不记录批次完成状态；阶段级结论只在 [roadmap](./I38-agent-roadmap.md) §2 写一次。本清单**不管理提交**。
> 范围：前端集成（共同视图 + 双布局）、审批 UI、Agent 模式。后端控制层归 P2（[I38a](./I38a-tool-control-layer-tasklist.md)），OS 工具归 P5（[I38c](./I38c-os-tools-tasklist.md)）。
> 测试位置：前端单测放 `src/**/__tests__/`；Rust 集成测试放 `src-tauri/crates/assistant-tools/tests/` 或 `src-tauri/tests/`。

---

## C1 DTO 契约（依赖 P4）

实现：`src-tauri/src/protocol/dto/agent.rs`、`src-tauri/src/protocol/dto/chat.rs`、`src-tauri/src/protocol/dto/settings.rs`、`src/types/protocol.contract.ts`

- [x] C1.1 `ToolApprovalRequestDto`：`conversationId` / `turnId` / `callId` / `toolName` / `arguments` / `intents` / `timeoutMs` / `context`
- [x] C1.2 `ApprovalContextDto` 为唯一包裹：`tier` / `behavior` / `reasons` / `preview` / `extensions`；顶层无这四个字段（A12）
- [x] C1.3 `ApprovalReasonDto` 五类（`defaultPolicy` / `approvalLevel` / `projectRule` / `tierCeiling` / `advisor`）、`ApprovalPreviewDto`、`ApprovalExtensionDto` 与 `ApprovalExtensionSeverityDto`
- [x] C1.4 `AgentToolCallPayloadDto` 变体：`started` / `argsDelta` / `finished` / `failed` / `awaitingApproval` / `approvalClosed` / `awaitingAnswer`
- [x] C1.5 `AgentModeDto.kind` 枚举改为 `chat` / `agent`；新增 `toolConfig: ConversationToolConfigDto`（输出恒在，缺省取契约缺省 `sandbox` / `none` / 空名单）；`AgentModeInputDto` 同步（输入侧 `toolConfig` 可选）
- [x] C1.6 `ConversationDto.modeId`
- [x] C1.7 `ConversationToolConfigDto` **落库**：`ConversationDto.toolConfig`（键恒在、可 `null`）+ `ConversationPatchDto.toolConfig`（缺省/`null` = 不改）
- [x] C1.7b `SendChatMessageArgsDto.toolConfig` 语义改为「每轮有效配置」（agent 链路 = 会话持久值 ?? 模式初始值；chat 链路恒 `null`）
- [x] C1.8 `npm run contract:gen` 生成 `src/types/protocol.contract.ts`
- [x] C1.9 `cargo test --test assembly_contract` 通过；`protocol_bindings` DTO 计数断言同步

---

## C1a 模式模型收敛 + 手动迁移（依赖 C1；规格 §4.1 / §4.3 / §4.11）

实现：`src-tauri/crates/assistant-storage/src/agent_modes.rs`、`src-tauri/crates/assistant-storage/src/db.rs`、`src-tauri/crates/assistant-agent/src/prompt_builder.rs`、`src-tauri/src/protocol/dto/settings.rs`、`src/settings/agentMode/types.ts`、`src/settings/core/agentKind/agentModeDefinitions.ts`

- [x] C1a.1 `agent_modes` 内置种子改为三行：`builtin-default`(chat) / `builtin-translate`(chat) / `builtin-agent`(agent)；删除 `builtin-quick` / `builtin-deep`
- [x] C1a.3 `conversations` 加列 `tool_config_json TEXT`：同上幂等迁移
- [x] C1a.4 `AgentModeRow` / `ConversationRow` 新增 `tool_config_json: Option<String>`；`MODE_SELECT` / `insert` / `update` / `reset_builtin_mode` 同步
- [x] C1a.5 `AgentKind` 枚举改为 `Chat`（`Default`）/ `Agent`；`from_mode_kind` **只认 `agent`**：`agent` → `Agent`，其余（`chat` / `default` / `quick` / `deep` / `custom` / `null` / 未知）→ `Chat`；`custom` 不做映射
- [x] C1a.6 `supports_tool_call` 改为 `matches!(self, AgentKind::Agent)`；工具注入仍要求 `enabledTools` 非空（kernel 已把空清单归一为 `None`）
- [x] C1a.7 前端 `AgentMode.kind` 类型改为 `"chat" | "agent"`；`agentModeDefinitions` 移除 `quick` / `deep` 分组，保留 `chat` / `agent`；分组判定为 `kind === "agent"` → agent 组，其余（含旧值）→ chat 组；自定义模式创建时二选一、编辑时锁定（旧值行打开编辑按 `chat` 展示，保存即写 `chat`）
- [x] C1a.8 手动迁移 SQL 落地（§4.11）：内置行 `kind` 归一（按 `id`）、会话改指、删除 `builtin-quick` / `builtin-deep`，**并手动 `DROP COLUMN` 删除旧工具三列** `tools` / `max_tool_tier` / `approval_level`（一次性、不可逆）；**不做**旧 `kind` 值重映射；执行前备份、执行后跑校验查询（A24 / A25）
- [x] C1a.9 已迁移库上重复执行 `init_db` 与迁移 SQL 无副作用（幂等单测）
- [ ] C1a.10 自定义模式双形态：`kind = chat` 走 chat 路径（无工具 UI），`kind = agent` 走 agent 路径（可装配工具）；`tool_config_json` 为 `NULL` 时取缺省 `sandbox` / `none` / `[]` / `[]`；旧 `custom` 行不享受 agent 形态（仅兼容标签）。**例外**：`builtin-agent` 的内置种子刻意写 `sandbox` / `dangerous_only`（见 `BUILTIN_AGENT_TOOL_CONFIG`），属模式级初始值，不是缺省兜底

---

## C1b 链路选路（依赖 C1、C1a）

实现：`src/chat/core/conversationLink.ts`、`src/services/chatOrchestrator.ts`、`src/chat/ConversationView.tsx`

- [x] C1b.1 只有 `kind === "agent"` 归 agent 链路；`chat` / 旧值（`default` / `quick` / `deep` / `custom`）/ `null` / 其余未知值 / mode 行缺失一律归 chat 链路，单一实现
- [x] C1b.2 发送时按会话 `modeId` 判定，**不取全局选中态**
- [x] C1b.3 `toolConfig` 只对 agent 链路构造，chat 恒为 `null`
- [x] C1b.4 ~~会话内锁定链路：已有消息后选择器只展示会话自身 `modeId`~~ **作废（2026-11-07 决策调整）**：改为会话内可切换，见 C1d；C1b 只保留「选择器展示会话当前 `modeId`」
- [x] C1b.5 单测 `conversationLink.test.ts` 覆盖各类 `modeId`（`agent` / `chat` / 旧值 `default`、`quick`、`deep`、`custom` / `null` / 未知值 / 行缺失），断言只有 `agent` 进 agent 链路（A11）
- [ ] C1b.6 chat 会话不渲染工具 UI（A11）——依赖 C3 / C4 组件落地后校验
- [ ] C1b.7 自定义 `kind = chat` 模式与内置「对话」走完全相同的 chat 路径（命令 / `TurnSpec` / 布局三者一致）（A11）

---

## C1c 审批信息装配（依赖 C1）

实现：`src-tauri/crates/assistant-tools/src/gateway.rs`、`src-tauri/crates/assistant-tools/src/policy.rs`、`src-tauri/crates/assistant-tools/src/os/path.rs`、`src-tauri/src/adapters/approval.rs`、`src-tauri/src/setup.rs`

- [x] C1c.1 `ApprovalRequest` 增加单个 `context: ApprovalContext`
- [x] C1c.2 `Policy::explain` 产出命中规则；`ask_reasons` 只保留 `Ask` 效果并去重（A15）
- [x] C1c.3 `Tool::preview_for_approval` 端口（默认 `None`）+ 2s 超时包装，超时/失败降级 `None`（A13 / A14）
- [x] C1c.4 仅在 `Ask` 路径调用预演；`Allow` / `Deny` 路径调用计数为 0（A14）
- [x] C1c.5 `extensions` 默认空数组，检测方按需填充；工作区检测复用 `ensure_within_root`（A17）
- [x] C1c.6 替换 `DenyAllApprovalPrompter` 为 `TauriApprovalPrompter`（`TimingApprovalPrompter` 装饰器不动）
- [x] C1c.7 审批超时改读 `approvalTimeoutSecs`（`null` → 120s，`Some(0)` 拒绝）（A18）
- [x] C1c.8 `workspace_root` 按配置 → `ToolUseContext` 接入，每轮现取（A17）
- [x] C1c.9 `delete` 预演与 `execute` 共用 `scan_tree`，量级数字一致（A16）

---

## C1d 共同视图 + 双布局（依赖 C1、C1b；规格 §4.8 / §4.10）

实现：`src/chat/ConversationView.tsx`、`src/chat/layouts/ChatLayout.tsx`、`src/chat/layouts/AgentLayout.tsx`、`src/chat/selectors/ModeSelector.tsx`、`src-tauri/src/cmd/conversation.rs`

- [x] C1d.1 `ChatView` 的会话级逻辑（状态读取 / send / cancel / retry / regenerate / branch / fork / compact / 附件）原样迁入 `ConversationView`，语义不改
- [x] C1d.2 `ChatLayout`：对话 chrome + 消息区 + 输入区 + `ContextBar`，不引用任何工具 / 审批 / 提问组件
- [ ] C1d.3 `AgentLayout`：复用共享组件（`MessageList` / `MessageInput` / `AttachmentPreview` / `ContextBar` / `CompactBanner` / 选择器），仅追加工具气泡、审批 / 提问卡片、`ToolToolbar` + `ToolConfigPanel`（共享框架与插槽已就位；插槽内容依赖 C3 / C4 / C5 组件落地）
- [x] C1d.4 布局由 `resolveConversationRoute(modeId, modes).link` 决定，**不按消息内容 / 全局选中态**判定；切换模式只换布局，不重建视图
- [x] C1d.5 模式切换为 `ModeSelector` 单控件：下拉按 `kind` 分组列出对话组 / Agent 组，选中项直接决定链路（跨组与组内是同一动作）；不设分段切换控件，无组内记忆内存态
- [x] C1d.6 默认 chat：新建会话 `modeId = builtin-default`；`builtin-agent` 恒存在，Agent 组永不禁用
- [x] C1d.7 turn 进行中（phase 非 `idle` / `error`）切换控件置灰；后端 `patch_conversation` 返回 `conversation_busy` 时原样提示
- [x] C1d.8 移除 `patch_conversation` 的消息数守卫与 `error.conversation_mode_locked` i18n，新增 `error.conversation_busy`
- [x] C1d.9 `ChatView` 下线：`OrbPanel` / 主窗口改挂 `ConversationView`（保留 `modelOpenRef`），全仓无 `ChatView` 残留引用
- [ ] C1d.10 验收 A11 / A21 / A22 / A23（A21 / A22 已满足；A11 / A23 的工具 UI 与装配面板依赖 C3 / C4 / C5）
- [x] C1d.11 输入草稿提升为 `ConversationView` 受控值（按 `conversationId` 索引），布局替换不丢草稿
- [x] C1d.12 `resolveEffectiveToolConfig`（会话级 ?? 模式级 ?? 缺省）单一实现；首次进入 agent 布局且会话级为 `null` 时播种并落库；`updateToolConfig` 唯一写入路径（失败回滚 + 提示）

---

## C2 前端事件接入（依赖 C1）

实现：`src/services/chatStreamEvents.ts`、`src/chat/core/chatTypes.ts`、`src/stores/`

- [x] C2.1 订阅 `session-state-changed`（P4 已接线）
- [x] C2.2 订阅 `agent-tool-call` 并把 `payload` 写入消息模型
- [x] C2.3 按 `turnId` 过滤迟到事件，旧代际丢弃（A3）
- [ ] C2.4 多窗口同一会话 phase 一致（A3）

---

## C3 ToolCallBubble 五态（依赖 C2）

实现：`src/chat/tool/`（`ToolCallBubble` 与样式）、`src/chat/layouts/AgentLayout.tsx`（插槽）

- [x] C3.1 五态渲染：`started` / `finished` / `failed` / `awaitingApproval` / `approvalClosed`
- [x] C3.2 大结果折叠，`persisted-output` 引用 + 预览
- [x] C3.3 历史消息只出现 `success` / `error` 终态（A1）
- [x] C3.4 单测覆盖事件映射与终态收敛（A1）

---

## C4 审批 UI（依赖 C3、C1c）

实现：`src/chat/tool/`（`ApprovalCard`）、`src-tauri/src/cmd/`（审批命令）、设置页「已授权操作」

- [x] C4.1 `resolve_agent_tool_call` 命令接线；`approve` / `deny` / `allowSession` / `allowAlways` 四态回填（A2）
- [x] C4.2 超时置灰，前端不重复提交（A2）
- [x] C4.3 三授权动作文案：`[允许]` / `[本会话允许]` / `[始终允许]`；本会话同键免询问，全局同键免询问且二次确认（A2）
- [x] C4.4 L0 / L1 / L2 分区渲染：`tier` / `behavior` / `reasons`（非空）/ `preview` / `extensions`（A12）
- [x] C4.5 `preview = null` 不显示「0 个条目」；`truncated = true` 显示「≥ N」；`estimated` 恒提示（A13）
- [x] C4.6 `extensions` 按 `severity` 通用渲染，未知 `kind` 不丢弃、`value` 纯文本（A12b）
- [x] C4.7 设置页「已授权操作」列出 `permission_grants` 行（作用域标签 + 会话归属），支持单条撤销、作用域修改（会话级⇄全局）与全部清除（A18）

---

## C5 前端编排上移（依赖 P4 稳定）

实现：`src/services/chatOrchestrator.ts`、`src/services/chatStreamEvents.ts`、`src/stores/chatMessageSlice.ts`、`src/stores/chatConversationSlice.ts`、`src/stores/chatStreamStore.ts`

- [x] C5.1 删除 `ensureIdle()`（含 `ENSURE_IDLE_TIMEOUT_MS` 轮询与 `resendFrom` / `switchConversation` 两处调用）
- [x] C5.2 切会话不取消后台 turn：`onConversationSwitch` 只换本地流式视图，事件按 `conversationId` 路由；切回运行中会话由 `resumeStream` 恢复（`messageId` 由首个增量补全）
- [x] C5.3 前端 phase 自维护改为后端 `session-state-changed` 驱动：按会话记录 phase / turnId，`send` / 文本增量不再推导 phase
- [x] C5.4 显式停止仍可取消（A4）：`cancel()` 仍按会话链路分派 `cancel_*`，并清理该会话的 phase / turnId 记录

---

## C6 AgentMode UI（依赖 C1）

实现：`src/settings/pages/AgentModeSettings.tsx`、`src/chat/tool/ToolToolbar.tsx`、`src/chat/tool/ToolConfigPanel.tsx`、`src/chat/layouts/AgentLayout.tsx`（工具栏插槽）

- [x] C6.1 设置页表单**移除**层级上限 / 审批级别 / 工具多选（改为只读「默认装配摘要 + 在对话中调整」提示）
- [x] C6.2 设置页表单保留 persona 配置：名称 / 描述 / 类型（创建时二选一、编辑锁定）/ prompt / 模型 / 温度 / maxTokens / 推理
- [x] C6.3 内置模式不可删除；编辑时锁定 `name` / `kind`，并提供「恢复默认」（A8）
- [x] C6.4 `ToolConfigPanel`（仅 `AgentLayout`）：按 tier 分组的工具多选（Sandbox / OS / External），默认全不勾选；变更即 `patch_conversation { toolConfig }` 落库（A8）
- [x] C6.5 `ToolToolbar`（仅 `AgentLayout`）：当前会话可用工具概览条，点击展开 `ToolConfigPanel`
- [x] C6.6 `resolveAvailableTools` 前端调用与 UI 绑定（后端已由 P5 完成）
- [x] C6.7 工具可用性变更提示（「已启用 shell」等系统消息）
- [x] C6.8 「设为该模式默认」按钮：写回 `agent_modes.tool_config_json`，只影响未来新建会话；仅自定义 agent 模式与 `builtin-agent` 显示
- [x] C6.9 后端 `update_agent_mode` 校验 `kind` 不可变（与既有行不一致返回 `error.agent_mode_kind_locked`）；前端编辑态类型控件只读

---

## C7 模式到策略映射（依赖 C6、P2）

实现：`src-tauri/crates/assistant-tools/src/policy.rs`、`src-tauri/src/adapters/`、项目级 `.assistant/permissions.json`

- [x] C7.1 `policy_from_agent_mode`：`none` 改写基座规则为 `("*","*",Allow)`；`dangerous_only` 保持默认；`all_os_tools` 在基础规则内追加 `("read","*",Ask)`
  - 单层内为 last-match-wins，`none` 必须**改写**首条基座而非追加（追加到末尾会连硬保护 Deny 一起覆盖）
  - `all_os_tools` 的宽泛 `read` 规则必须排在硬保护规则之前，否则「读敏感路径」的 Deny 会被降级成 `Ask`
  - `default_global_rules()` 拆为 `base_rules()`（可被审批级别改写）+ `hard_deny_rules()`（恒 Deny），拼接结果与原 7 条规则同序
- [x] C7.2 tier 约束层：`sandbox` 拒绝 `tier:os` / `tier:external`；`os` 拒绝 `tier:external`；`external` 无约束
  - `ResolvedPermissionPolicy::evaluate_tier_ceiling()` **只看约束层**：主规则集的 `("*","*")` 会匹配任意资源串，参与判定会把「默认需审批」误判为层级拒绝
  - `Gateway::evaluate` 在 intent 判定之前先查层级上限，越界直接 `Denied` 并写审计（A7 的第二道闸）
- [x] C7.3 单测：三档审批级别的规则差异（A10）
  - `policy.rs` 内 `mod tests`：`none_level_allows_non_protected_actions` / `dangerous_only_keeps_default_reads_allowed` / `all_os_tools_asks_even_for_reads` / `approval_level_rules_are_explained_as_approval_level`
  - `RuleOrigin::ApprovalLevel` 使审批卡片的 `reason` 输出 `approvalLevel` 而非投影成 `projectRule`
- [x] C7.4 单测：tier 规则拒绝超层级工具（A10）
  - `sandbox_ceiling_denies_os_and_external_tiers` / `os_ceiling_denies_only_external_tier` / `external_ceiling_denies_nothing`
  - `none` 级别下 tier 上限仍生效：自动执行不等于越过层级
- [x] C7.5 项目级 `.assistant/permissions.json` 加载与合并顺序
  - `src-tauri/src/adapters/permission_policy.rs`：`policy_for_tool_config()` 按「默认 → 审批级别 → tier → 项目层」组装
  - 项目层只作为额外约束层追加，经 `most_restrictive` 合并，**只能收紧不能放宽**
  - 文件缺失视为无项目规则；读取失败 / JSON 非法 → 整层退化为 `("*","*",Deny)`（fail-closed）
  - `src-tauri/tests/permission_policy.rs` 覆盖缺失、收紧、不可放宽、非法回退、tier 来源

---

## C8 `ask_user` 提问交互（依赖 C3、C1c；**不属 P6 主体退出条件**）

实现：`src-tauri/crates/assistant-tools/src/os/agent_ask_user.rs`、`src-tauri/src/cmd/`、`src/chat/tool/`（`QuestionCard`）

- [x] C8.1 `AgentQuestionRequestDto` 与 `awaitingAnswer` 载荷（DTO 已落地）
  - `protocol/dto/agent.rs` 追加 `AgentToolCallPayloadDto::AwaitingAnswer`；前端 `toolCallView` 把 `awaitingAnswer` 与 `awaitingApproval` 统一映射到 `awaiting_user`
- [x] C8.2 `Tool::manages_own_execution_timeout()`（默认 `false`）+ `exec::invoke` 分支不套统一 deadline（A19）
  - 默认实现返回 `false`，存量工具零改动；`exec::invoke` 对自管超时工具跳过 30s 统一 deadline，取消仍由 turn 子令牌在外层立即收口
- [x] C8.3 `agent_ask_user` 工具实现（`AGENT_ONLY`），1–4 问、`multiSelect`（A20）
  - `assistant-tools/src/os/agent_ask_user.rs`：`ToolTier::Sandbox` + `AGENT_ONLY` + `manages_own_execution_timeout() = true`
  - 输入校验集中在 `assistant-tools/src/question.rs`（1–4 问、每问 2–10 选项、`header` ≤20 字符），工具与适配层复用同一套规则
- [x] C8.4 `userQuestionTimeoutSecs` 配置：`null` → 180s、`0` → 不限时（A19）
  - `AppConf.user_question_timeout_secs: Option<u32>`；`setup.rs` 每次提问现读配置，读取失败降级 180s（不静默变无限等待）
  - 设置页「Agent 工具」组新增「提问等待上限（秒）」输入框，`min=0`（与审批的 `> 0` 不同，`0` 合法）
- [x] C8.5 `resolve_agent_question` 命令接线
  - `adapters/question.rs`：`PendingQuestionRegistry`（`call_id` 主键 + `conversation_id` 跨会话校验 + 题目数量校验）+ `TauriQuestionPrompter`
  - 缺路由信息 / 重复 `call_id` 直接 `Cancelled`，不把 turn 挂到超时；超时与取消由 `PendingQuestionGuard` 幂等清理登记项
- [x] C8.6 提问卡片：逐条渲染、「其他」写 `other`、全部作答才可提交、超时置灰、历史只显示终态（A20）
  - `src/chat/tool/QuestionCard.tsx`：`timeoutMs = 0` 不显示倒计时；倒计时结束只置灰，收口等后端 `failed(cancelled)` 事件
  - 历史消息不渲染卡片，`ask_user` 按 `success` / `error` / `cancelled` 终态展示

---

## 验收命令

`cwd = src-tauri` 与仓库根分别执行。

- [x] C9.1 `npm run contract:gen` 后 `src/types/protocol.contract.ts` 可编译（A9）
- [x] C9.2 `cargo test --test assembly_contract` 通过（A9）
- [x] C9.3 `cargo clippy --workspace --all-targets -- -D warnings` 无告警
- [x] C9.4 先执行格式化修复，再 `cargo fmt --all --check` 通过
- [x] C9.5 `npm run lint` / `npm run typecheck` / `npm run test` 通过
- [ ] C9.6 手工回归：审批三态、超时置灰、多窗口 phase、同会话模式切换（空闲可切、turn 进行中置灰、多窗口被后端拒绝）、切回 agent 复用工具配置、重启后会话级装配恢复

---

## 验收映射

| 规格验收 | 对应任务 |
|---|---|
| A1 五态与历史终态 | C3.1、C3.3、C3.4 |
| A2 审批三态与超时 | C4.1、C4.2、C4.3 |
| A3 phase 一致与旧事件丢弃 | C2.3、C2.4 |
| A4 删除 `ensureIdle()` | C5.1–C5.4 |
| A5 `ChatDeltaPayloadDto` 无工具字段 | C1.4（变体独立于 delta） |
| A6 deny-by-default | C6.4、C6.6 |
| A7 `maxToolTier` 硬上限 | C7.2、C7.4 |
| A8 表单默认全不勾选、内置不可改 | C6.3、C6.4 |
| A9 契约生成与编译 | C1.8、C1.9、C9.1、C9.2 |
| A10 审批级别映射与 tier 拒绝 | C7.1–C7.4 |
| A11 链路选路与会话内切换 | C1b.1–C1b.6、C1d.4–C1d.8 |
| A12 单 `context` 包裹 | C1.2、C4.4 |
| A12b `extensions` 通用渲染 | C4.6 |
| A13 `preview` 降级文案 | C1c.3、C4.5 |
| A14 预演仅 `Ask` 路径 | C1c.3、C1c.4 |
| A15 `explain` 与 `evaluate` 一致 | C1c.2 |
| A16 预演与执行共用扫描 | C1c.9 |
| A17 工作区边界同源 | C1c.5、C1c.8 |
| A18 审批超时配置与撤销入口 | C1c.7、C4.7 |
| A19 `ask_user` 超时独立 | C8.2、C8.4 |
| A20 提问卡片 | C8.3、C8.6 |
| A21 视图 / 布局边界 | C1d.1–C1d.3、C1d.9 |
| A22 默认 chat 与模式切换 | C1d.5、C1d.6 |
| A23 会话级工具配置持久化 | C1d.12、C9.6 |
| A24 模式模型收敛与自定义双形态 | C1a.1、C1a.7、C1a.10 |
| A25 手动迁移幂等与旧值兜底 | C1a.5、C1a.8、C1a.9 |
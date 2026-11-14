# I43 — Assistant 消息身份指示规格（头像 / 模式 / 模型）

| 项 | 值 |
|---|---|
| 状态 | 现行 |
| 日期 | 2026-11-09 |
| 修订 | 2026-11-10：按当前代码核对后修订（历史页目录加载、历史树身份规则、avatar 三态表单映射、chat 链路 `mode_id` 来源、验收方式、影响面补齐）；同日取消 CSP 放开：删除远程 / `asset:` 头像分支，改为「不改 CSP + 渲染层拒绝 URL 前缀 + 后续走 `data:` 内联图」；同日将身份指示布局由「头像列 + 内容列」两列改为气泡**上方独立一行**（头像在行首、不占用气泡侧边，气泡无左缩进）；同日完成全部自动化断言并 UI 手工验收通过（A9–A13、A18、A21），I43 实现与验收闭环 |
| 关联 | I38d（已归档）（共同视图 / 双布局 / 消息气泡）、I38h（已归档）（`conversations.mode_id` 会话绑定）、I38 agent roadmap §4 |
| 影响范围 | `assistant-storage`（`messages` / `agent_modes` 表与行结构）、`assistant-agent`（`CreateAssistantReq`）、`src-tauri/src/protocol/dto`（`MessageDto` / `AgentModeDto` / `AgentModeInputDto`）、`src-tauri/src/cmd`（落库入口）、`src-tauri/src/adapters`（落库适配）、`src/chat/message/`（气泡与头像）、`src/settings/pages/`（模式设置页）、`src/history/`（历史树身份解析的目录加载）、`src/i18n/locales/`、前端与 Rust 测试夹具 |

> 本文只定义「assistant 消息的发送方身份指示」与「消息级模式持久化」。气泡排版、工具卡片、审批五态仍以 I38d（已归档）为准，本文不重复。
> 本文不新增阶段；它是跨 P6（前端视图）与 P12（会话绑定）的**契约变更**，按 roadmap §7 第 7 条同时落在两个阶段的既有实现上。

---

## 1 目标与非目标

### 1.1 目标

| # | 交付 |
|---|---|
| 1 | assistant 消息显示发送方身份：头像 + 模式名 + 模型名 |
| 2 | 头像按**模式**配置，v1 只支持内置 SVG |
| 3 | 模式信息**精确到消息**：每条 assistant 消息落库时固化 `mode_id` |
| 4 | 同一轮次（turn）内多条 assistant 消息只渲染一次身份指示（列表型视图，见 N2） |
| 5 | 历史消息在会话切换模式后仍显示当时的模式身份 |
| 6 | 身份缺失时按固定降级链兜底，不出现空白占位 |

### 1.2 非目标

- 用户消息的身份指示（用户侧不做任何改动）。
- 头像文件上传、资产安装、图片裁剪等资产通道。
- Rive / GIF / 视频头像；`src/mascot/renderers/` 的吉祥物渲染器不参与消息头像。
- **任何 URL 型头像**：v1 不支持 `http://` / `https://` / `asset:` 前缀的头像，渲染层也不为它们生成 `<img src>`（见 §5.3 第 5 项）。
- **CSP 与 `assetProtocol` 的任何改动**：本文不修改 `tauri.conf.json` 的 `security.csp` 与 `security.assetProtocol`。现有 `img-src 'self' asset: data: blob:` 已足以覆盖 v1 需求（内置 SVG + `data:` 内联图）。放开 `img-src https:` 会打开无 CORS 约束的出站通道，且 v1 没有任何远程头像需求，故不在范围内。
- 按模型维度配置头像（v1 只有模式维度）。
- 修改 turn 内模式切换规则（I38d 已定：turn 进行中禁止切换）。
- 为历史树详情引入轮次归并（历史树逐行渲染，无「可见序列」概念，见 §5.4 与 N7）。

### 1.3 前置

1. I38d（已归档）的双布局与 `ConversationFrame → MessageList → MessageBubble` 共用链路已稳定。
2. I38h（已归档）的 `conversations.mode_id` 已落库，会话级模式可读。
3. `npm run contract:gen` / `contract:check` 可用。
4. 消息级 `provider_id` / `model_id` 已落库并已在 wire 上（见 §3.6）。

### 1.4 后续扩展路径（不在 v1 实现，仅记录决策）

v1 只有内置 SVG。将来若支持自定义头像，按成本 / 风险排序：

| 优先级 | 方案 | 单次成本 | 是否需改 CSP |
|---|---|---|---|
| 1 | 扩充内置 SVG | 0（随 bundle 解析） | 否 |
| 2 | `data:` 内联图存库 | ≈ 0 | **否**，`img-src` 已含 `data:` |
| 3 | `asset:` 协议 + 专用目录 | 0.1–1 ms（协议分发 + 作用域校验 + 读盘） | 否，只需扩 `assetProtocol.scope` |
| 4 | 远程 HTTPS | 50–200 ms（DNS + TCP + TLS，负载 2–4 KB 仅约 0.3 ms） | 是，需 `img-src` 主机白名单 |

结论：自定义头像优先走第 2 条（`data:`），在完全不动安全配置的前提下即可实现；第 3 条作为图片过大时的备选。第 4 条只有在「头像必须云端同步」时才有意义，且必须写成显式主机白名单，禁止裸 `https:`。

---

## 2 术语与不变量

| 术语 | 定义 |
|---|---|
| 身份指示 | 头像 + 模式名 + 模型名，三者共同回答「这条 assistant 消息是谁发的」 |
| 消息级模式 | `messages.mode_id`，assistant 消息落库时写入的模式 id，此后不再随会话模式变化 |
| 轮次起点 | 可见消息序列中，`role === "assistant"` 且前一条可见消息 `role !== "assistant"` 的那条消息 |
| 可见消息序列 | `MessageList` 过滤掉 `role === "tool"` 之后的序列（与现有 `visibleMessages` 同口径） |

不变量：

| # | 不变量 |
|---|---|
| N1 | 只有 `role === "assistant"` 的消息参与身份指示；`user` / `tool` / `compact_summary` 一律不渲染头像与身份行 |
| N2 | **仅限列表型视图（`MessageList`）**：一个轮次只渲染一次身份指示，位置在该轮次第一条可见 assistant 消息上；同轮次后续 assistant 消息不渲染身份行（头像与文字一并省略）。因身份行独立位于气泡上方、气泡无左缩进，省略后气泡左缘天然对齐，无需宽度占位。历史树详情不受此约束，见 N7 |
| N3 | 身份指示的模式来源必须是 `message.mode_id`，**禁止**用 `conversation.mode_id` 或当前选中的模式推断历史消息 |
| N4 | `messages.mode_id` 一旦写入不随模式编辑、删除、重命名而改变；模式被删除后仍保留原 id 文本 |
| N5 | 头像缺失、模式缺失、模型缺失都不得产生空白占位或抛出异常，必须走 §5.3 降级链 |
| N6 | 消息级模式与 `role` 无关地保留在 wire 与本地类型上（`user` 行同样可为 `null`，但不读取） |
| N7 | 历史树详情（`HistoryBranchRow`）逐行渲染树节点，**不存在**「可见消息序列」与「轮次」概念：每条 `assistant` 行都渲染身份指示，不得为历史树伪造轮次分组，也不得把 N2 套用到历史树 |

---

## 3 数据模型

### 3.1 数据库

| 表 | 列 | 类型 | 默认 | 说明 |
|---|---|---|---|---|
| `agent_modes` | `avatar` | `TEXT` | `NULL` | 头像标识，`NULL` 表示未配置 |
| `messages` | `mode_id` | `TEXT` | `NULL` | 该消息所属模式 id；`NULL` 表示未知（存量行） |

迁移规则（`src-tauri/crates/assistant-storage/src/db.rs`）：

1. 新库建表分支：两条 `CREATE TABLE` 语句内直接声明上述列（`agent_modes` 建表语句末尾追加 `avatar TEXT`，`messages` 建表语句末尾追加 `mode_id TEXT`）。
2. 旧库升级分支：沿用现有「`pragma_table_info` 存在性检查 + `ALTER TABLE ... ADD COLUMN`」写法，列存在即跳过；`ALTER TABLE` 只追加列，不设 `NOT NULL`、不设默认值。
3. **不回填**存量数据：历史 assistant 消息的 `mode_id` 保持 `NULL`，前端按 §5.3 兜底；不为存量行补 `avatar`。
4. 两列的迁移彼此独立，各自一个 `pragma_table_info` 检查块，不得合并成一次 `execute_batch`。

### 3.2 存储行结构

| 位置 | 变更 |
|---|---|
| `agent_modes.rs` `AgentModeRow` | 新增 `avatar: Option<String>` |
| `agent_modes.rs` `MODE_SELECT` | 当前 15 列（`row.get(0)?` … `row.get(14)?`），末尾追加 `avatar` 成为第 16 列，`read_mode_row` 是**下标取值**，必须同步追加 `row.get(15)?` |
| `agent_modes.rs` `insert_mode` / `update_mode` | 参数与占位符同步追加 `avatar`：`insert_mode` 当前 `?1..?15` → 补 `?16`；`update_mode` 当前到 `?14` → 补 `?15` |
| `agent_modes.rs` `builtin_defaults` | 三个内置模式各自给出默认 `avatar`：`builtin-default` → `builtin:chat`、`builtin-agent` → `builtin:agent`、`builtin-translate` → `builtin:translate`，见 §5.2 清单 |
| `agent_modes.rs` `reset_builtin_mode` | 该 `UPDATE` 当前只到 `?11`（`tool_config_json=?11`），追加 `avatar=?12`，重置时回到内置默认值 |
| `conversations/mod.rs` `MessageRow` | 新增 `mode_id: Option<String>` |
| `conversations/mod.rs` `MSG_SELECT` | 末尾追加 `mode_id`（第 17 列）；`read_msg_row` 是**列名取值**（`row.get("id")` 等），只需新增 `row.get("mode_id")?`，无下标顺序约束 |
| `conversations/msg_ops.rs` `insert_message` | 复用 `MSG_SELECT` 拼 `INSERT`，占位符列表补 `?17`，`params!` 补 `row.mode_id`（**必须放在最后**，与列顺序一致） |
| `conversations/branch.rs` 新建分支消息的 `INSERT` | 该语句自带显式列清单与 `VALUES`，需追加 `mode_id` 列与 `?10` 绑定值 |
| `conversations/ops.rs` `duplicate_conversation` / `fork_to_new_conversation` | 构造 `MessageRow` 时补 `mode_id: msg.mode_id.clone()`——**复制原值，不置空**，否则副本丢失历史身份（违反 N4 语义） |
| `src-tauri/src/cmd/compaction.rs` | 摘要行补 `mode_id: None` |
| `src-tauri/src/cmd/chat/insert.rs` | 两处 `MessageRow` 字面量补 `mode_id`（user 行 `None`，assistant 占位行按 §3.4 取值） |
| `src-tauri/src/adapters/message_store.rs` | 两处 `MessageRow` 字面量补 `mode_id`（透传 `CreateAssistantReq.mode_id`） |

> 注意：`insert_message` 的列顺序由 `MSG_SELECT` 决定，`MSG_SELECT` 同时被 `SELECT` 复用。追加新列只能放在**末尾**，禁止插到中间，否则所有按位置绑定的 `INSERT` 会整体错位。

### 3.3 DTO（字段约束，权威源为 `src-tauri/src/protocol/dto/*.rs`）

| DTO | 字段 | 方向 | 约束 |
|---|---|---|---|
| `AgentModeDto` | `avatar: Option<String>` | 输出 | 不加 `skip_serializing_if`：键恒在，值可为 `null`；前端类型为 `string \| null` |
| `AgentModeInputDto` | `avatar: Option<String>` | 输入 | `#[serde(default)]` + `#[ts(optional = nullable)]`；三态语义见 §3.5 |
| `MessageDto` | `mode_id: Option<String>` | 输出 | 不加 `skip_serializing_if`：键恒在，值可为 `null`；前端类型为 `string \| null` |
| `MessageDto` | `provider_id` / `model_id` | 输出 | **已存在，不新增**：消息级模型来源已落库并已在 wire 上，见 §3.6 |
| `CreateAssistantReq`（内核端口，非 wire） | `mode_id: Option<String>` | 内部 | 由调用方传入本轮模式 id；内核不推断 |

契约生成：

1. 改完 Rust DTO 后执行 `npm run contract:gen`，提交 `src/types/protocol.contract.ts`。
2. 本次不新增 DTO 类型，`src-tauri/tests/protocol_bindings.rs` 的 `declarations` 数组无需新增条目（仅改既有类型字段）。
3. `AgentModeInputDto` 的 `avatar` 必须同步进 `src/stores/agentModeStore.ts` 的 `toModeInput()`，否则前端全量提交会丢失该字段（映射规则见 §6）。

### 3.4 落库写入点（assistant 消息的 `mode_id` 来源）

| 入口 | 文件 | 写入值 |
|---|---|---|
| agent 迭代新建消息（占位行与提交行） | `assistant-agent/src/agent/loop_executor/persist.rs` → `CreateAssistantReq.mode_id` | `ctx.input.mode.as_ref().map(\|m\| m.id.clone())`；同一 turn 所有迭代取值相同 |
| agent 适配层落库 | `src-tauri/src/adapters/message_store.rs` → `create_assistant_message` | 透传 `CreateAssistantReq.mode_id` |
| chat 链路 `user` 行（普通追加与分支插入） | `src-tauri/src/cmd/chat/insert.rs` | `NULL` |
| chat 链路 `assistant` 占位行（普通追加与分支插入） | `src-tauri/src/cmd/chat/insert.rs` | 本次发送的 `SendChatMessageArgsDto.mode_id`；为空时回退 `conversations.mode_id` |
| 分支新建（历史树 fork） | `conversations/branch.rs` | `assistant` 行继承 `conversations.mode_id`；`user` 行写 `NULL` |
| 会话复制 / 分叉复制 | `conversations/ops.rs` | 复制源行的 `msg.mode_id`（不重置） |
| 压缩摘要消息 | `src-tauri/src/cmd/compaction.rs` | `NULL`（`compact_summary` 不渲染身份） |

规则：

1. 同一 turn 的所有 assistant 迭代消息必须取**同一个** `mode_id`（turn 起点快照），禁止每次迭代重读当前选中模式。内核侧只能从 `TurnInput.mode` 取，不得读 DB。
2. `mode_id` 为空字符串视为 `NULL`，不做格式校验；模式 id 的合法性由模式表外键逻辑之外的调用方保证（现有模式表无外键约束）。
3. chat 链路的普通追加与分支插入**共用同一来源**（`SendChatMessageArgsDto.mode_id`，空则回退 `conversations.mode_id`），不得出现两条路径各自取值的分歧。
4. `insert_chat_messages` 需新增入参 `mode_id: Option<&'a str>`，并透传给 `insert_user_message_with_attachments`（固定传 `None`）与 `insert_placeholder_assistant_message`（传解析后的值）。两个私有函数的签名同步扩展。

### 3.5 值语义

`avatar`：

| 值 | 含义 |
|---|---|
| `NULL` / 缺省（输出） | 未配置，走 §5.3 降级链 |
| `builtin:<name>` | 内置 SVG 头像，`<name>` 取自 §5.2 清单 |
| `data:image/png;base64,...` / `data:image/jpeg;...` / `data:image/webp;...` | 内联图片数据；CSP `img-src` 已含 `data:`，无需改安全配置。长度上限与格式白名单见 §5.2 |
| `http://` / `https://` / `asset:` 前缀 | **不支持**：渲染层不生成 `<img src>`，直接走 §5.3 第 5 项兜底；保存时不做前缀校验，但落库后也不会产生出站请求 |
| 其它任意字符串 | 视为未知标识，按降级链处理，不报错 |

`avatar` 输入三态（`save_agent_mode`）：

| 输入 | 语义 |
|---|---|
| 缺省 / `null` | 沿用库内现值（与 `tool_config` 同口径） |
| `""`（空串） | 清除为 `NULL` |
| 非空 | 覆盖为给定值 |

`mode_id`：

| 值 | 含义 |
|---|---|
| `NULL` | 未知来源（存量消息、压缩摘要、user 行），前端按兜底展示 |
| 非空 | 该消息落库时生效的模式 id；模式删除后仍保留原值 |

### 3.6 模型来源（已存在，不改造）

身份行的模型段**不需要新增字段**：消息级模型来源在现有实现中已落库、已上线 wire、已进前端类型。

| 层 | 现状 |
|---|---|
| 数据库 | `messages.provider_id`、`messages.model_id` 两列已存在（`MSG_SELECT` 已含） |
| 存储行 | `MessageRow` 已有 `provider_id: Option<String>`、`model_id: Option<String>` |
| wire | `MessageDto` 已有 `provider_id` / `model_id`（`Option<String>`，不加 `skip_serializing_if`，键恒在） |
| 前端类型 | `Message = Omit<MessageDto, "toolCalls" \| "status"> & {...}`，自动带 `providerId` / `modelId` |
| agent 链路 | `persist.rs::ensure_row` 由 `TurnRunCtx.provider_id` / `model_id` 填 `CreateAssistantReq`，`message_store.rs` 透传落库 |
| chat 链路 | `cmd/chat/insert.rs` 只对 **`assistant` 占位行**写 `provider_id` / `model_id`；`user` 行写 `NULL` |
| 例外 | 压缩摘要行、`user` 行（chat 链路与历史树 fork 均同）写 `NULL`（不参与身份渲染） |

因此本节只做两件事：

1. **不新增任何模型字段**，前端直接用 `message.modelId` / `message.providerId`。
2. **模型展示名解析**：`modelId` 是模型 id，不是展示名；解析走 `useProviderStore.providers`，见 §4.3。

> 注意：`modelId` 的取值是**该条消息实际请求的模型**，与 `conversation.modelId`（会话当前模型）不同。会话中途换模型后，历史消息仍显示当时模型；这与 N3 对模式的要求一致，无需额外改造。

---

## 4 前端数据流与判定

### 4.1 类型

| 位置 | 变更 |
|---|---|
| `src/types/protocol.contract.ts` | 生成结果自带 `MessageDto.modeId: string \| null`、`AgentModeDto.avatar: string \| null`、`AgentModeInputDto.avatar?: string \| null` |
| `src/chat/core/chatTypes.ts`（`Message` 定义处） | `Message` 由 `MessageDto` 派生，自动获得 `modeId`；本地扩展字段保持交叉类型写法 |
| `src/stores/agentModeStore.ts` | `toModeInput()` 补 `avatar`（映射规则见 §6） |

`MessageDto` 新增 `mode_id` 后，`modeId` 在 `Message` 上变为**必填**（`string | null`），因此**所有 `Message` 字面量都必须补该字段**，否则 `npm run typecheck` 直接失败。生产代码与测试夹具一并补齐：

| 文件 | 位置 | 补值 |
|---|---|---|
| `src/chat/core/optimisticMessage.ts` | `createOptimisticMessage()` | **乐观占位消息的唯一构造点**，身份字段缺省值（`null`）与 `modeId` / `providerId` / `modelId` 入参都收敛在此 |
| `src/services/stream/turnOrchestrator.ts` | 全部乐观消息构造（错误占位、分支点 assistant、普通追加 user / assistant） | 一律走 `createOptimisticMessage()`，不写字面量；取值见下方「取值规则」 |
| `src/stores/agent/agentSegments.ts` | `makePlaceholder()` | 本次发送模式 id |
| `src/stores/chatCompactSlice.ts` | `manualCompact` 的 `summaryMsg` | `modeId: null`（压缩摘要不渲染身份） |
| `src/services/__tests__/turnOrchestrator.test.ts` | 消息夹具 | `modeId: null` |
| `src/stores/agent/__tests__/agentSegments.test.ts` | 2 处夹具 | `modeId: null` |
| `src/stores/__tests__/chatPendingQuestions.test.ts` | 1 处夹具 | `modeId: null` |
| `src/chat/core/__tests__/toolCallView.test.ts` | 3 处夹具 | `modeId: null` |
| `src/chat/core/__tests__/protocolContract.test.ts` | 1 处夹具 | `modeId: null` |

取值规则（`turnOrchestrator.ts`，经 `createOptimisticMessage()` 落值）：

1. `modeId` 取 `deps.env.getConversationRoute(convId).modeId`（`ConversationRoute` 已带 `modeId`，见 `src/chat/core/conversationLink.ts`）；无会话的错误占位消息填 `null`。
2. `providerId` / `modelId` 取 `deps.env.getSelectedProviderId()` / `deps.env.getSelectedModelId()`。
3. **读取顺序必须提前**：`getConversationRoute(convId)`、`getSelectedProviderId()`、`getSelectedModelId()` 在构造乐观消息**之前**一次性读入局部变量，后续构造与 `SendChatMessageArgs` 复用同一组值，禁止重复调用 env 端口。
4. **不得**引用 `ConversationView` 的 `currentProviderId` / `currentModelId`：那是 React 组件状态，`turnOrchestrator` 是纯 service，只能通过 `TurnEnvPort` 取值。

乐观占位消息补 `modeId` / `providerId` / `modelId` 是必需项：流式期间消息尚未落库回读，若不填则首屏身份指示与落库后不一致（类型不报错，但视觉会跳）。

### 4.2 轮次起点判定

在 `MessageList` 内按可见消息序列计算，作为 prop 传入 `MessageBubble`：

```
showIdentity(index) = visibleMessages[index].role === "assistant"
                     && (index === 0 || visibleMessages[index - 1].role !== "assistant")
```

约束：

1. 判定只依赖相邻两条**可见**消息的 `role`，不使用时间戳、`turn_id` 或 `parent_id`。
2. `tool` 行在 `visibleMessages` 中已过滤，因此「连续 assistant 块」等价于「同一 turn 的多迭代序列」。
3. 分页加载在头部 prepend 时，当前页第一条若无前驱则按起点渲染；这是允许的展示冗余，不做跨页对齐。
4. `compact_summary` 不是 `assistant`，其后的 assistant 消息重新显示身份。
5. 本判定**只适用于 `MessageList`**。历史树详情（`HistoryBranchRow`）逐行渲染、无可见序列上下文，一律按 N7 显示身份。

### 4.3 身份解析

| 输入 | 解析 |
|---|---|
| 模式 | `message.modeId` → `useAgentModeStore.modes` 中查找同 id 项 → 取 `name` / `avatar` / `kind` |
| 模式缺失 | `modeId` 为 `null`，或在 `modes` 中查不到（已删除）→ 名称显示 i18n 兜底文案，头像按 `kind` 未知走中性兜底 |
| 模型 | `message.modelId` + `message.providerId` → `useProviderStore.providers` 中按 `providerId` 找 provider、再按 `modelId` 找 model，取 `model.name \|\| model.id`（与 `ModelSelector` 同口径）；provider 或 model 已不存在时回退原始 `modelId`；`modelId` 为空则省略模型段，不留占位 |
| 模式名过长 | 单行省略号截断，完整文本进 `title`（tooltip） |

解析在渲染层完成，禁止在 store 里缓存「消息 → 模式 / 模型」映射：模式与 provider 目录均可编辑，身份文字需随编辑更新，而 `modeId` / `modelId` 本身不变。

**目录加载前置**（新增约束）：身份解析依赖 `useAgentModeStore.modes` 与 `useProviderStore.providers` 两个目录。二者当前只在主会话链路与设置页加载，`src/history/` 下没有任何加载调用。因此：

| 渲染面 | 加载点 | 现状 |
|---|---|---|
| 主会话视图 | `ConversationView.tsx` 挂载时 `loadProviders()` 与 `loadModes()` | 已具备 |
| 历史树详情 | `HistoryManager.tsx` 挂载时 `void loadModes(); void loadProviders();` | **缺失，必须新增** |

不加载的后果：从历史页直接进入时两个目录为空，所有历史消息退化成「未知模式 + 原始 `modelId`」，目标 5 不成立。加载失败按空目录处理，走 §5.3 降级链，不阻塞历史页渲染。

### 4.4 上下文与 prop

| 位置 | 变更 |
|---|---|
| `MessageBubble.tsx` | 新增可选 prop `showIdentity?: boolean`，缺省 `true`（历史页等无列表上下文的场景直接显示身份） |
| `MessageList.tsx` | 按 §4.2 计算后显式传入；现有 `RoleContext`（`prevRole`）保持不变，仅用于分组间距 |
| `HistoryBranchRow.tsx` | 不传 `showIdentity`，走缺省 `true`；同时不传 `compact`，故尺寸取 20px（见 §5.1）。历史树逐行渲染，每条 assistant 行都显示身份（N7） |

---

## 5 渲染规范

### 5.1 布局

assistant 消息根节点保持**纵向单列**；身份指示独立成行，位于气泡**上方**（不占用气泡侧边）：

| 元素 | 规格 |
|---|---|
| 根容器（assistant） | 保持 `.root` 的 `display: flex; flex-direction: column`，`align-items: flex-start`；**不改为横向两列** |
| 身份行 | 消息根节点第一行，`display: flex; align-items: center; gap: 4px`；横向顺序为「头像 → 模式名 → 分隔符 → 模型名」，整行 `max-width: 100%`，`margin-bottom: 4px` |
| 头像尺寸 | 单一变量 `--msg-avatar-size`；每面**只有一个值生效**，见下表 |
| 气泡 | 紧接身份行之后，位于 `.stack` 内，**无左缩进**；内部结构（内容块 / 推理块 / 工具卡片 / 状态行）不变 |
| 身份行排版 | 单行；字号 11px，行高 = `--msg-avatar-size`（头像与文字垂直居中对齐），`white-space: nowrap`，溢出省略号 |
| 身份行颜色 | 弱化前景（现有 muted 文本色），不使用强调色 |
| 时间戳 | 位于底部操作行的行首（`.actionsWrapper` 内），与分支选择器、操作按钮同一行；整行 `justify-content: flex-end`，右缘对齐气泡右缘，随操作按钮一起 hover 显示 |
| 用户消息 | 根容器保持原纵向布局（`.rootRoleUser` 的 `align-items: flex-end` 不变），无身份行、无左缩进 |

尺寸取值（`compact` 不是可切换的两种模式，而是按渲染面固定）：

| 渲染面 | `compact` 实际取值 | 来源 | 头像 / 身份行高度 |
|---|---|---|---|
| 主会话视图 | 恒 `true` | `ConversationFrame.tsx` 直接传 `compact`（布尔简写），`MessageList` 无其它调用点传 `false` | 18px |
| 历史树详情 | 恒 `false` | `HistoryBranchRow.tsx` 调用 `MessageBubble` 时不传 `compact`，走缺省 | 20px |

因此**不要**在文档或代码里表述为「默认 20px，compact 时 18px」的两态切换：主会话面永远只出现 18px，20px 只在历史树详情生效。实现上仍保留 `--msg-avatar-size` 单一变量：在 `.rootRoleAssistant` 作用域内声明 20px，在 `.compact` 作用域内覆盖为 18px，避免两处硬编码。

高度预算（以主会话面 18px 为准）：身份行位于**气泡之上、消息根节点内部**，不改变 `.root + .root` 的 `margin-top: 4px` 与 `.rootGroupGap` 的 `margin-top: 12px`，因此不与既有间距叠加。每轮次净增高 = 身份行高度（18px）+ 身份行与气泡间距（4px），约 22px；非起点消息不渲染身份行，不新增高度。这与 I38d（已归档）「agent 一次迭代 = 一条消息 = 一个独立气泡」的边界规则不冲突：气泡本身高度与左缘位置不变，只是首条气泡上方多一行身份。

### 5.2 头像组件

新增 `src/chat/message/MessageAvatar.tsx` 与 `MessageAvatar.module.css`：

| 项 | 规格 |
|---|---|
| props | `modeId: string \| null`、`size?: number`（缺省由 CSS 变量 `--msg-avatar-size` 决定） |
| 数据源 | `useAgentModeStore((s) => s.modes.find((m) => m.id === modeId))`，单条选择，避免整表订阅 |
| 解析顺序 | 见 §5.3 |
| 无障碍 | `aria-hidden="true"`（身份由相邻文字表达，头像不重复朗读） |
| 缓存 | 组件内不缓存模式对象；模式编辑后需即时反映 |
| 渲染容器 | 正方形、圆角（与现有视觉语言一致）、`flex: 0 0 auto`、`overflow: hidden` |

内置 SVG 集合放在 `src/chat/message/avatars/index.tsx`，导出 `BUILTIN_AVATARS`（`name → Component`）与 `BUILTIN_AVATAR_OPTIONS`（供设置页 select 使用）。v1 清单：

| `builtin:<name>` | 用途 | 默认使用者 | 视觉 |
|---|---|---|---|
| `builtin:chat` | `kind === "chat"` 的默认头像 | `builtin-default` 的种子值 + 降级链第 4 项 | 对话气泡轮廓，蓝色系 |
| `builtin:agent` | `kind === "agent"` 的默认头像 | `builtin-agent` 的种子值 + 降级链第 3 项 | 机器人 / 齿轮轮廓，紫色系 |
| `builtin:translate` | 翻译模式默认头像 | `builtin-translate` 的种子值（该模式 `kind` 同为 `chat`，靠显式 `avatar` 区分，不靠降级链） | 双向箭头，青色系 |
| `builtin:default` | 未知模式的最终兜底 | 降级链第 5 项 | 中性几何形状，灰蓝色 |

> `builtin:translate` 与 `builtin:chat` 的分工必须明确：翻译模式的 `avatar` 列种子值写 `builtin:translate`；只有 `avatar` 为 `NULL` 时才落到按 `kind` 推导的 `builtin:chat`。二者不冲突，因为降级链只在 `avatar` 不可用时生效。

SVG 一律使用 `currentColor` 或固定主题色，不引入图片资源与外部依赖；尺寸由外层容器控制，SVG 自身 `width/height: 100%`。

`data:` 内联图（v1 可选能力，默认数据不会出现）：

| 项 | 规格 |
|---|---|
| 格式白名单 | 仅 `data:image/png;base64,`、`data:image/jpeg;base64,`、`data:image/webp;base64,`；`image/svg+xml` 与其它 MIME 一律按未知标识兜底 |
| 长度上限 | 序列化后 ≤ 8 KB；超出按未知标识兜底，不报错 |
| 渲染 | `<img src={avatar} aria-hidden="true">`，`object-fit: cover`；`onError` 回落 §5.3 第 3 项之后的逻辑 |
| 来源 | 只允许由设置页写入；v1 不提供入口，故正常情况下库内不会出现 |

### 5.3 降级链

按顺序取第一个可用项：

| 顺序 | 条件 | 结果 |
|---|---|---|
| 1 | `avatar` 以 `data:image/png;base64,` / `data:image/jpeg;base64,` / `data:image/webp;base64,` 开头，且长度 ≤ 8 KB | `<img src={avatar}>`（CSP `img-src` 已含 `data:`，无需改配置） |
| 2 | `avatar` 为 `builtin:<name>` 且 `<name>` 在 `BUILTIN_AVATARS` 中 | 渲染对应内置 SVG |
| 3 | 模式存在，`kind === "agent"` | `builtin:agent` |
| 4 | 模式存在，`kind === "chat"` | `builtin:chat` |
| 5 | 其余：模式缺失 / 未知 `kind` / `modeId === null` / `http(s)` 或 `asset:` 前缀 / 非白名单 `data:` / 超长 `data:` / 非内置未知串 | `builtin:default` |

第 1 项的 `img` 加载失败回落用组件内 `onError` 切换到第 3 项之后的逻辑，不重试、不打日志到用户可见区域。

第 5 项**显式吸收** `http://` / `https://` / `asset:` 三类前缀：渲染层对它们**不得**生成任何 `<img src>`，保证即使库内被写入外部 URL（手工改库、导入旧数据）也不会产生出站请求。这是「不改 CSP」之外的运行时兜底，两层都不可省略。

### 5.4 状态矩阵

| 场景 | 头像 | 身份行 | 说明 |
|---|---|---|---|
| 单轮单迭代 | 显示 | 显示 | 常规路径 |
| 单轮多迭代（agent，列表视图） | 仅首条显示 | 仅首条显示 | 其余消息整行省略身份（N2），气泡无左缩进 |
| 流式进行中 | 显示（来自乐观占位消息的 `modeId`） | 显示 | 与落库后一致 |
| 消息中断 / 错误 | 显示 | 显示 | 身份与状态无关 |
| `compact_summary` | 不显示 | 不显示 | 不是 assistant |
| `user` 消息 | 不显示 | 不显示 | 用户侧不做改动 |
| 模式已删除 | 兜底 SVG | 兜底文案 | `modeId` 保留原值 |
| `modeId === null`（存量消息） | `builtin:default` | 兜底文案 + 模型段（若有） | 历史遗留，不回填 |
| 历史树详情（`HistoryBranchRow`） | 每条 assistant 行都显示 | 同左 | 逐行渲染，无轮次归并（N7）；`compact` 为 `false`，尺寸 20px |
| 历史页首次进入（目录未加载完成） | 按 `kind` 未知走 `builtin:default` | 兜底文案 + 原始 `modelId` | 目录加载完成后自动更新，不缓存解析结果 |
| 头部加载更多后 | 按 §4.2 判定 | 同左 | 允许页边界重复显示 |
| 主会话视图 | 18px | 18px 行高 | `compact` 恒为 `true`，见 §5.1 尺寸表 |

---

## 6 设置页（模式头像配置）

`src/settings/pages/AgentModeSettings.tsx`：

| 项 | 规格 |
|---|---|
| 表单字段 | 在现有 `fieldSchemas` 新增 `avatar` 项，位置放在「名称 / 描述」之后、「提示词」之前 |
| 控件 | `select`（`type: "select"`），选项 = `BUILTIN_AVATAR_OPTIONS` + 「未设置」空值项 |
| 空值选项 | 「未设置」必须使用**显式** `value: ""`，不得依赖 `allowClear` 或 `undefined`（antd 清空产出 `undefined`，会落进「沿用现值」分支） |
| 空值语义 | 选中「未设置」→ 提交 `""`，后端清除为 `NULL`，渲染走 §5.3 第 3 项之后 |
| 表单值 → 输入 DTO 映射 | `toModeInput()` 必须**原样透传** `avatar`：`undefined → undefined`、`null → null`、`"" → ""`。**禁止**沿用其它字段的 `?? null` 兜底，否则 `""` 会被折叠为 `null`（= 沿用现值），头像永远清不掉 |
| 预览 | 选项右侧或表单下方展示所选头像的 20px 预览；无选择时展示按 `kind` 兜底的预览 |
| 保存 | `saveMode` 走全量 `toModeInput()`，`avatar` 必须出现在 `toModeInput()` 返回值中 |
| 内置模式重置 | `reset_builtin_mode` 恢复 §3.2 中该内置模式的默认 `avatar` |
| 删除模式 | 不影响已落库消息的 `mode_id`；消息头像回落 §5.3 第 5 项 |
| URL / 图片输入 | v1 不提供任何 URL 或图片输入项，只有内置头像下拉；后续自定义头像走 §1.4 第 2 条（`data:` 内联图），CSP 无需改动 |

i18n 新增 key（`zh-CN.json` / `en-US.json` 同步）：

| key | 用途 |
|---|---|
| `settings.agent_mode.field.avatar.label` | 字段标题 |
| `settings.agent_mode.field.avatar.description` | 字段说明（内置头像，暂不支持上传） |
| `settings.agent_mode.avatar.none` | 「未设置」选项 |
| `chat.message.identity.unknown_mode` | 模式缺失时的兜底名称 |
| `chat.message.identity.separator` | 模式名与模型名之间的分隔符（如 `·`） |
| `chat.message.identity.avatar.builtin_chat` / `builtin_agent` / `builtin_translate` / `builtin_default` | 设置页选项显示名 |

---

## 7 禁止项

| # | 禁止 |
|---|---|
| 1 | 用当前选中模式或 `conversation.modeId` 推断历史消息身份 |
| 2 | 在 `MessageBubble` 内读 `useChatStore` 的当前会话模式或 `conversation.modelId` 替代 `message.modeId` / `message.modelId` |
| 3 | 为用户消息渲染头像或身份行 |
| 4 | 引入头像上传、资产安装、远程拉取缓存（v1 范围外）；为 `http://` / `https://` / `asset:` 前缀生成 `<img src>`；改动 `tauri.conf.json` 的 `security.csp` 或 `security.assetProtocol` |
| 5 | 复用 `src/mascot/renderers/` 的吉祥物组件作为消息头像 |
| 6 | 通过 `as unknown as` 转换 DTO 或头像值；类型不符时修正类型定义 |
| 7 | 在文档里重复声明 DTO 完整类型（权威源是 Rust DTO + 生成产物） |
| 8 | 在 `messages` 表迁移里回填存量 `mode_id` 或写 `NOT NULL` 约束 |
| 9 | 同一 turn 内按迭代重新读取模式 id（必须用 turn 起点快照） |
| 10 | 用 `replace_all` 批量改落库 SQL 占位符（逐文件按序修改） |
| 11 | 把 `MSG_SELECT` 的新列插到中间（必须追加在末尾，否则 `insert_message` 的位置绑定整体错位） |
| 12 | 为 `user` 行（chat 链路 / fork / 复制）写非 `NULL` 的 `mode_id` |
| 13 | 在历史页未加载 `modes` / `providers` 目录时就依赖身份解析结果做判断（必须先加载，见 §4.3） |
| 14 | 用 `undefined` 表示「未设置」头像，或在 `toModeInput()` 里用 `?? null` 抹掉空串 |
| 15 | 在历史树详情套用「轮次归并」逻辑（历史树逐行渲染，见 N7） |

---

## 8 验收标准

> 测试方式说明：当前项目 `vitest` 未配置 `environment: "jsdom"`，未安装 `@testing-library/react`，`src/` 下不存在 `.test.tsx`。因此涉及 DOM 渲染的条目一律**手工验收**；若后续引入组件测试基建，可另立条目转为自动化，本文不把该基建列为 v1 范围。
>
> 验收状态：A1–A8、A14、A17、A22 已由自动化断言覆盖并通过；A9–A13、A18、A21 已于 2026-11-10 完成 UI 手工验收并通过。

| # | 验收 | 方式 |
|---|---|---|
| A1 | 新库建表包含 `messages.mode_id` 与 `agent_modes.avatar` | 存储层单测：建库后 `pragma_table_info` 断言 |
| A2 | 旧库升级只追加列、列存在时跳过、存量行值为 `NULL` | 存储层单测：预置旧 schema 后跑迁移 |
| A3 | `insert_message` / `read_msg_row` 往返保留 `mode_id` | 存储层单测：插入 `Some("builtin-agent")` 后读回相等 |
| A4 | `agent_modes` 增删改查与重置保留/恢复 `avatar` | 存储层单测：`reset_builtin_mode` 后等于内置默认值（含 `builtin-translate → builtin:translate`） |
| A5 | agent 一轮多迭代的所有 assistant 行 `mode_id` 相同且等于 turn 起点模式 | 内核集成测试：多迭代 turn 后逐行断言 |
| A6 | chat 链路 `user` 行 `mode_id` 为 `NULL`，`assistant` 行为发送模式 | 命令层内联单测（`src-tauri/src/cmd/chat/insert.rs`）：user 行 `NULL`、assistant 行取发送模式、空串/缺省回退 `conversations.mode_id`、分支插入同规则 |
| A7 | `MessageDto` 输出恒含 `modeId` 键（值可为 `null`） | 序列化断言 |
| A8 | 契约生成与校验通过 | `npm run contract:gen` + `npm run contract:check` |
| A9 | 单轮多迭代（主会话 `MessageList`）只出现一个头像与一行身份 | 手工验收（自动化需先引入组件测试基建） |
| A10 | `user` / `compact_summary` 消息不渲染身份 | 手工验收 |
| A11 | 模式缺失、头像未配置、`modeId` 为 `null` 三种情况均渲染 `builtin:default` 且不抛错 | 手工验收 |
| A12 | 会话中途切换模式后，历史消息头像仍为旧模式 | 手工验收（`messages[0].modeId` 与 `conversation.modeId` 不同时按消息值渲染） |
| A13 | 设置页可保存、清除、重置 `avatar` | 设置页手工验收 + `save_agent_mode` 单测（三态） |
| A14 | 类型检查与 lint 通过 | `npm run typecheck`、`npm run lint`、`cargo clippy` |
| A15 | 会话中途换模型后，历史消息模型段仍为旧模型；模型已下线时回退原始 id | 手工验收 |
| A16 | 模型展示名取 `model.name`，缺失时回退 `model.id`；`modelId` 为 `null` 时省略模型段且不留空位 | 手工验收 |
| A17 | 主会话视图头像恒为 18px、历史树详情恒为 20px，不存在同一面内两种尺寸并存 | 断言 `.rootRoleAssistant` 声明 20px、`.compact` 覆盖为 18px（CSS 声明值 / class 断言）；**jsdom 不做布局计算，不得断言实际像素尺寸** |
| A18 | 历史页直接进入（不先进会话、不进设置）时，历史消息仍显示模式名与模型展示名 | 手工验收：冷启动后直接打开历史详情，确认目录已加载 |
| A19 | 历史树详情每条 assistant 行都显示身份（不因同轮多迭代而合并） | 手工验收 |
| A20 | 历史页在目录加载失败/为空时不崩溃、不白屏，走兜底文案 | 手工验收 |
| A21 | 库内 `avatar` 被手工写入 `https://…` 或 `asset://…` 时，渲染 `builtin:default` 且不产生任何网络请求 | 手工验收：改库后观察开发者工具无出站请求 |
| A22 | `tauri.conf.json` 的 `security.csp` 与 `security.assetProtocol` 未被修改 | `git --no-pager diff src-tauri/tauri.conf.json` 无该两节变更 |

---

## 9 实施步骤

按依赖顺序执行。**每步完成后可编译**的前提是：该步涉及的结构体字面量与测试夹具必须同批补齐（完整清单见 §10），否则该步不成立。

| # | 步骤 | 主要文件 |
|---|---|---|
| C1 | DB 迁移：两表加列（新库建表 + 旧库 ALTER，两个独立检查块） | `assistant-storage/src/db.rs` |
| C2 | `agent_modes` 行结构、SELECT、insert/update、内置默认值（含 translate）、重置 | `assistant-storage/src/agent_modes.rs` |
| C3 | `MessageRow`、`MSG_SELECT`（追加末尾）、`insert_message`、分支 INSERT、会话复制/分叉复制（复制 `mode_id`）、摘要行 | `assistant-storage/src/conversations/{mod,msg_ops,branch,ops}.rs`、`src-tauri/src/cmd/compaction.rs` |
| C4 | DTO 字段（`AgentModeDto` / `AgentModeInputDto` / `MessageDto`）与 `save_agent_mode` 三态 | `src-tauri/src/protocol/dto/{settings,chat}.rs`、`src-tauri/src/cmd/agent_mode.rs` |
| C5 | `CreateAssistantReq.mode_id`、内核落库透传（`ctx.input.mode`，**不改 `TurnRunCtx`**）、适配层透传 | `assistant-agent/src/kernel/ports.rs`、`assistant-agent/src/agent/loop_executor/persist.rs`、`src-tauri/src/adapters/message_store.rs` |
| C6 | chat 插入签名扩展（`InsertChatInput` / `MessageIdentity` / 三个私有函数）、分支赋值、摘要置空、A6 内联单测 | `src-tauri/src/cmd/chat/insert.rs`、`src-tauri/src/cmd/compaction.rs` |
| C7 | 契约生成与前端类型同步（`toModeInput` 补 `avatar`；`createOptimisticMessage` 单一构造点补 `modeId` / `providerId` / `modelId`，读取顺序提前；`chatCompactSlice` 与全部测试夹具补 `modeId`） | `npm run contract:gen`、`src/stores/agentModeStore.ts`、`src/chat/core/optimisticMessage.ts`、`src/services/stream/turnOrchestrator.ts`、`src/stores/agent/agentSegments.ts`、`src/stores/chatCompactSlice.ts`、`src/**/__tests__/*.ts` |
| C8 | 头像组件与内置 SVG 集合（4 个内置头像 + 选项常量 + `data:` 分支 + URL 前缀拒绝） | `src/chat/message/MessageAvatar.tsx`、`.module.css`、`src/chat/message/avatars/index.tsx` |
| C9 | 气泡身份行（气泡上方独立一行，纵向单列，不改为两列）、`MessageList` 起点判定 | `src/chat/message/MessageBubble.tsx`、`MessageBubble.module.css`、`MessageList.tsx` |
| C10 | 设置页头像字段（显式空值选项）与 i18n（中英同步） | `src/settings/pages/AgentModeSettings.tsx`、`src/i18n/locales/*.json` |
| C11 | 历史页目录加载（`modes` + `providers`） | `src/history/HistoryManager.tsx` |

---

## 10 影响面清单

| 层 | 文件 | 改动性质 |
|---|---|---|
| 存储 | `src-tauri/crates/assistant-storage/src/db.rs` | 加列迁移（2 处独立检查块 + 2 处建表声明） |
| 存储 | `src-tauri/crates/assistant-storage/src/agent_modes.rs` | 行结构 + SELECT + 下标读取 + 内置默认 + 重置 |
| 存储 | `src-tauri/crates/assistant-storage/src/conversations/mod.rs` | 行结构 + `MSG_SELECT`（末尾追加）+ 列名读取 |
| 存储 | `src-tauri/crates/assistant-storage/src/conversations/msg_ops.rs` | 插入占位符 `?17` + 参数 |
| 存储 | `src-tauri/crates/assistant-storage/src/conversations/branch.rs` | 显式列清单 + `?10` 绑定 |
| 存储 | `src-tauri/crates/assistant-storage/src/conversations/ops.rs` | `duplicate_conversation` / `fork_to_new_conversation` 复制 `mode_id` |
| 内核 | `src-tauri/crates/assistant-agent/src/kernel/ports.rs` | `CreateAssistantReq` 加字段 |
| 内核 | `src-tauri/crates/assistant-agent/src/agent/loop_executor/persist.rs` | 两处 `CreateAssistantReq` 透传（取值 `ctx.input.mode`；`TurnRunCtx` 与 runner 构造点**不改动**） |
| 内核 | `src-tauri/crates/assistant-agent/src/kernel/ports/test_doubles.rs` | 测试桩字段同步 |
| 协议 | `src-tauri/src/protocol/dto/settings.rs` | 模式 DTO 加 `avatar` |
| 协议 | `src-tauri/src/protocol/dto/chat.rs` | 消息 DTO 加 `mode_id` |
| 命令 | `src-tauri/src/cmd/agent_mode.rs` | 三态保存 + 行结构字面量补 `avatar` |
| 命令 | `src-tauri/src/cmd/chat/insert.rs` | 入参扩展 + `MessageIdentity` 聚合身份字段 + 两处 `MessageRow`（`user` 行 `None`、`assistant` 占位行按 §3.4 取值）+ 内联单测（A6） |
| 命令 | `src-tauri/src/cmd/compaction.rs` | 摘要行 `MessageRow` 补 `mode_id: None` |
| 适配 | `src-tauri/src/adapters/message_store.rs` | 两处 `MessageRow` 透传 |
| 前端类型 | `src/types/protocol.contract.ts` | 生成产物 |
| 前端状态 | `src/stores/agentModeStore.ts` | `toModeInput` 补 `avatar`（原样透传） |
| 前端状态 | `src/stores/agent/agentSegments.ts` | `makePlaceholder` 补 `modeId` / `providerId` / `modelId` |
| 前端状态 | `src/stores/chatCompactSlice.ts` | 压缩摘要占位消息补 `modeId: null` |
| 前端流 | `src/chat/core/optimisticMessage.ts` | **新增**：乐观占位消息单一构造点（身份字段缺省 `null`） |
| 前端流 | `src/services/stream/turnOrchestrator.ts` | 改用 `createOptimisticMessage` 补三字段 + 读取顺序提前 |
| 前端状态 | `src/stores/providerStore.ts` | **只读复用**：模型展示名解析，不改动 |
| 前端视图 | `src/chat/message/MessageList.tsx` | 起点判定 + 传 `showIdentity` |
| 前端视图 | `src/chat/message/MessageBubble.tsx` / `.module.css` | 身份行（气泡上方独立一行，纵向单列）+ `--msg-avatar-size` |
| 前端视图 | `src/chat/message/MessageAvatar.tsx` / `.module.css` | 新增（内置 SVG + `data:` 分支，不生成 URL 型 `src`） |
| 前端视图 | `src/chat/message/avatars/index.tsx` | 新增内置 SVG 与选项常量 |
| 前端历史 | `src/history/HistoryManager.tsx` | 挂载时加载 `modes` + `providers` |
| 前端设置 | `src/settings/pages/AgentModeSettings.tsx` | 头像字段（显式 `value: ""` 空值项） |
| 前端文案 | `src/i18n/locales/zh-CN.json` / `en-US.json` | 新增 key |
| 测试 | `src-tauri/tests/protocol_contract.rs` | `sample_message_row` 补 `mode_id` |
| 测试 | `src-tauri/tests/protocol_dto_roundtrip.rs` | `mode_row_without_tool_config` 补 `avatar` |
| 测试 | `src-tauri/crates/assistant-storage/tests/{common/mod,agent_modes}.rs` | `MessageRow` / `AgentModeRow` 夹具 |
| 测试 | `src-tauri/crates/assistant-agent/tests/{loop_executor,prompt_options_spec,kernel_request_tools,kernel_emit,kernel_runner,attachment_budget}.rs` | 行结构字面量 |
| **不改动** | `src-tauri/tauri.conf.json` | **明确排除**：`security.csp` 与 `security.assetProtocol` 保持原值，`img-src` 仍为 `'self' asset: data: blob:` |
| 测试 | `src/services/__tests__/turnOrchestrator.test.ts`、`src/stores/agent/__tests__/agentSegments.test.ts`、`src/stores/__tests__/chatPendingQuestions.test.ts`、`src/chat/core/__tests__/toolCallView.test.ts`、`src/chat/core/__tests__/protocolContract.test.ts` | `Message` 夹具补 `modeId` |
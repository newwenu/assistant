# I7 — Context Compaction 规格说明

## 1. 交付目标

上下文容量管理：当 token 用量达到阈值时，终止当前对话（只读），生成摘要存入 DB，前端显示"对话已达上限"横幅 + [新建对话] 按钮。新对话中模型可通过跨对话检索工具查询旧对话消息。

**设计原则**：
- **对话满即终止**：整个对话标记为已满、只读，不压缩续接
- **摘要即交接文档**：结构化摘要，用户复制粘贴到新对话后模型可直接继续工作
- **摘要消息可见**：摘要作为一条 `role: compact_summary` 的特殊消息插入消息流，前端通过 `MessageBubble` 渲染为交接摘要样式，用户可在对话中直接查看摘要内容
- **安全优先**：仅在 Provider 确认的 usage 数据下触发终止，无 usage 时不触发（避免误终止）

> **前置依赖**：I6 ✅（Agent 自定义模式 + token 追踪 + chatStore contextUsage 已就绪）
> **核心参考**：OpenBitFun `compression_prefetch.rs`（阈值判断）、deepseek-harness-orb `compaction-basic`（摘要策略 + 重试）

---

## 2. 触发机制

### 2.1 阈值判断

- 内置阈值 `COMPACTION_THRESHOLD_PERCENT = 80`
- 用户可通过 `compact_threshold_percent` 覆盖，默认使用内置常量值
- 判断条件：`usage.prompt_tokens >= context_window × threshold / 100`

### 2.2 触发时机与前提

- **时机**：每次 `send_chat_message` 流式输出结束后（`finish_reason` 到达）
- **前提**：流式响应中 `usage.prompt_tokens` 存在且大于 0
- **不触发**：Provider 未返回 usage、usage 为空、或 `auto_compact = false`

> **设计决策**：usage 缺失时不触发终止。原因：部分 Provider/网关不报告 usage，此时若从 DB 估算 token 总量，精度不足，可能造成意料外的过早结束对话。用户可在 Provider 返回 usage 的场景下获得可靠保护，在无 usage 场景下通过 ContextBar 百分比手动判断。

### 2.3 并发防护

终止判定通过后、摘要生成完成前，需阻止同一对话的后续 `send_chat_message`：

- 在内存中为对话设置"正在终止"标记（`terminating: HashMap<String, bool>`，按 conversation_id 隔离）
- `send_chat_message` 入口检查：`is_full` 或 `terminating` 为 true 时拒绝发送
- 摘要完成（无论成功或失败）后清除 `terminating` 标记

---

## 3. 终止执行流程

### 3.1 触发方式

终止流程有两种触发方式，共享同一套摘要生成 + 标记逻辑：

| 触发方式 | 入口 | 前提 |
|---------|------|------|
| **自动触发** | 流结束后 `should_terminate` 判定 | `usage.prompt_tokens` 存在 + `auto_compact=true` |
| **手动触发** | 用户点击 [生成交接摘要] 按钮 | 对话未满 + 无正在进行的流 + 无 terminating 标记 |

手动触发允许用户在任意位置主动生成摘要并终止对话，无需等待阈值。参考 deepseek-harness-orb 的 `/compact` 命令和 OpenBitFun 的 `compactNow()`。

### 3.2 流程步骤

1. **触发**：自动或手动 → 设置 `terminating` 标记
2. **插入摘要消息占位**：向消息列表插入 `role: compact_summary` 的空消息，通过 `chat-delta` 事件流式推送摘要内容到前端
3. **摘要生成**：调用当前 Provider 生成结构化摘要（详见 §4），流式填充到占位消息
4. **标记终止**：摘要成功 → 事务写入 `is_full=1` + 摘要到 DB → emit `conversation_terminated` 事件
5. **失败回滚**：摘要失败 → 清除 `terminating` 标记，对话继续可用，日志记录失败原因

### 3.3 关键约束

- **先摘要后标记**：摘要生成成功后才设置 `is_full=1`，失败不终止对话
- **事务写入**：`is_full` 更新与 `compression_state` 摘要写入在同一 SQLite 事务中，保证原子性
- **不阻塞流**：自动触发时当前流式输出已完成；手动触发时要求无正在进行的流

### 3.4 事件定义

**`conversation_terminated`** 事件 payload：

| 字段 | 类型 | 说明 |
|------|------|------|
| `conversation_id` | string | 被终止的对话 ID |
| `summary` | string | 摘要文本 |
| `is_full` | boolean | 确认对话已满 |

**`conversation_compact_failed`** 事件 payload（摘要生成失败时 emit）：

| 字段 | 类型 | 说明 |
|------|------|------|
| `conversation_id` | string | 对话 ID |
| `error` | string | 失败原因（超时 / Provider 错误等） |

**`conversation_unlocked`** 事件 payload（解锁对话时 emit）：

| 字段 | 类型 | 说明 |
|------|------|------|
| `conversation_id` | string | 被解锁的对话 ID |

---

## 4. 摘要生成

### 4.1 摘要定位

摘要为**可复制的交接文档**：用户粘贴到新对话后，模型无需额外检索即可继续基本工作。

参考 deepseek-harness-orb compaction-basic 的设计：
- 摘要替换最旧的平衡跨度，保留最近的对话尾部
- 摘要使用一次额外的模型请求生成
- 摘要必须自包含、具体、可操作

参考 airi compactor 的设计：
- 对工具调用保留工具名称和结果摘要
- 对文本内容截取关键片段
- 明确标注 `[Compacted]` 避免与原文混淆

### 4.2 摘要 Prompt

摘要 prompt 由两部分组成：**默认模板**（内置）+ **用户自定义覆盖**（可选）。

**默认模板**：

```
请为以下对话生成交接摘要，使新对话中的 AI 无需查阅原文即可继续工作。

输出格式（严格遵循）：

## 交接摘要

**目标**：用户的核心目标，1-2 句话

**进展**：
- 已完成的关键步骤，每步一句，按时间顺序

**待办**：
- 未完成的事项，如有

**关键信息**：
- 文件路径、配置值、变量名、错误码等后续必需的具体值
- 每条信息必须写具体值，禁止写"某个文件""某个值"等模糊表述

**工具调用摘要**：
- 已执行的工具名称及关键结果，每条一行
- 如：`read_file → config.rs 内容已读取`、`run_command → 编译通过`

规则：
1. 自包含：摘要中不引用"上文""之前"等指代，所有信息直接给出
2. 具体：文件路径写完整路径，变量名写全名，错误码写原始值
3. 精炼：不记录每步操作细节，只保留关键节点和决策
4. 可操作：粘贴到新对话后 AI 可立即继续，无需追问上下文
5. 工具调用：保留工具名称 + 结果摘要，省略完整输出
```

**用户自定义覆盖**：

- AppConf 新增 `compact_summary_prompt: Option<String>`，`None` 使用默认模板
- 用户可在设置中编辑自定义摘要 prompt，适配不同使用场景（编程、写作、数据分析等）
- 自定义 prompt 完全替换默认模板，非追加
- 默认值：`None`（使用内置默认模板）

### 4.3 摘要生成调用

- **前缀缓存复用**：不使用独立的 system prompt，而是重放对话的 system prompt + 前缀消息，将摘要指令作为最后一条 user message 追加。这样摘要调用是原始请求的前缀复用，Provider 的 KV cache 不会失效（参考 deepseek-harness-orb summarizer.ts）
- 使用当前对话的 Provider 和模型
- 以流式调用收集完整输出（兼容所有 Provider），设置 `max_tokens` 为用户配置的 `compact_summary_max_tokens` 或默认 4096
- 超时 30 秒，超时视为失败
- 失败时日志记录错误，清除 `terminating` 标记，对话继续可用，横幅提供 [重试] 按钮

### 4.4 摘要长度控制

- `MAX_SUMMARY_REQUEST_TOKENS = 4096`：内置默认值，用户可通过 `compact_summary_max_tokens` 覆盖
- `MAX_SUMMARY_OUTPUT_TOKENS = 2000`：内置截断阈值，不可覆盖
- 摘要生成后若超过 `MAX_SUMMARY_OUTPUT_TOKENS × 4`（即 8000 字符），截断并追加 `[摘要已截断，完整信息请通过 search_history 检索]`
- 截断确保摘要粘贴到新对话后不会占用过多上下文

---

## 5. 数据模型

### 5.1 conversations 表扩展

新增列：`is_full INTEGER NOT NULL DEFAULT 0`

| 值 | 语义 |
|----|------|
| 0 | 正常对话，可继续发送 |
| 1 | 已满对话，只读，`send_chat_message` 拒绝 |

`ConversationRow` 同步新增 `is_full: bool` 字段，`CONV_SELECT` 包含此列，`list_conversations` 返回此字段。

### 5.2 compression_state 表

已有表，结构不变（`conversation_id` + `last_compression_at` + `compression_count`）。

摘要不存储在此表中，而是作为一条 `role: compact_summary` 的消息插入 messages 表，通过消息流可见。前端通过 `conversation_terminated` 事件获取摘要文本。

### 5.3 DB 迁移方式

手动迁移：在 `init_db` 中通过 `pragma_table_info` 检测列是否存在，不存在则 `ALTER TABLE ADD COLUMN`。与现有 `mode_id` 列迁移方式一致。

---

## 6. 设置

### 6.1 AppConf 扩展

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `auto_compact` | bool | true | 自动终止开关 |
| `compact_threshold_percent` | u8 | 80 | 终止阈值百分比，默认使用内置常量 `COMPACTION_THRESHOLD_PERCENT` |
| `compact_summary_prompt` | Option\<String\> | None | 自定义摘要 prompt，None 使用默认模板 |
| `compact_summary_max_tokens` | u16 | 4096 | 摘要生成 max_tokens 上限，默认使用内置常量 `MAX_SUMMARY_REQUEST_TOKENS` |

Rust `AppConf` + TS `AppConf` interface 同步新增。

### 6.2 声明式设置注册

4 个设置项均通过 `defineSetting()` 注册到 `fieldDefinitions.ts`，group 为 `"app"`，section 为 `"general-main"`（通用设置页），由 `GeneralSettings.tsx` 的 `SettingsGroup` 渲染。

| key | type | group | section | default | label i18n key | 说明 |
|-----|------|-------|---------|---------|---------------|------|
| `auto_compact` | boolean | app | general-main | true | `settings.general.compact.auto_compact.label` | 自动终止开关 |
| `compact_threshold_percent` | slider | app | general-main | 80 | `settings.general.compact.compact_threshold_percent.label` | 终止阈值百分比，默认 80%；`visible` 依赖 `auto_compact=true` |
| `compact_summary_prompt` | textarea | app | general-main | null | `settings.general.compact.compact_summary_prompt.label` | 自定义摘要 prompt，null 使用默认模板；完全替换默认模板，非追加；`visible` 依赖 `auto_compact=true` |
| `compact_summary_max_tokens` | number | app | general-main | 4096 | `settings.general.compact.compact_summary_max_tokens.label` | 摘要生成 max_tokens 上限，默认 4096；`visible` 依赖 `auto_compact=true` |

**注册链路**：

1. `fieldDefinitions.ts` — `defineSetting()` 注册，导出为 `COMPACT_SETTINGS` 数组
2. `GeneralSettings.tsx` — `SettingsGroup` 的 `definitions` 属性追加 `COMPACT_SETTINGS`
3. `settingsDispatcher.ts` — `dispatchConfPatch("app", patch)` → `invoke("patch_conf")` → Rust `AppConf::amend()` → `AppConf::save()` → 跨窗口 `emitSettingEventToPeers`
4. `settingEvents.ts` — `ConfPatched` 事件消费 → `store.loadAppConf()` 刷新

**条件可见**：`compact_threshold_percent`、`compact_summary_prompt`、`compact_summary_max_tokens` 三个设置项设置 `visible: (conf) => !!conf.auto_compact`，仅在自动终止开启时显示。与现有 `MASCOT_COLOR_SETTING` 的 `visible` 模式一致。

**slider 配置**：`compact_threshold_percent` 使用 `min: 50, max: 99, step: 1, marks: { 50: "50%", 80: "80%", 99: "99%" }`，默认值 80。

**textarea 配置**：`compact_summary_prompt` 使用 `rows: 6`，`placeholder` 指向 i18n key `settings.general.compact.compact_summary_prompt.placeholder`（"留空使用默认模板"）。

**number 配置**：`compact_summary_max_tokens` 使用 `min: 512, max: 16384, step: 256`，默认值 4096。

### 6.3 i18n 键

设置相关 i18n 键注册在 `settings.general.compact` 命名空间下：

| i18n key | zh-CN | en-US |
|----------|-------|-------|
| `settings.general.compact.auto_compact.label` | 上下文上限自动终止 | Auto-terminate on context limit |
| `settings.general.compact.compact_threshold_percent.label` | 终止阈值 | Termination threshold |
| `settings.general.compact.compact_summary_prompt.label` | 自定义摘要提示词 | Custom summary prompt |
| `settings.general.compact.compact_summary_prompt.placeholder` | 留空使用默认模板 | Leave empty to use default template |
| `settings.general.compact.compact_summary_max_tokens.label` | 摘要最大长度 | Max summary length |

对话满横幅相关 i18n 键注册在 `chat.compact` 命名空间下：

| i18n key | zh-CN | en-US |
|----------|-------|-------|
| `chat.compact.banner_title` | 对话已达上下文上限 | Conversation reached context limit |
| `chat.compact.copy_summary` | 复制摘要 | Copy summary |
| `chat.compact.new_conversation` | 新建对话 | New conversation |
| `chat.compact.unlock` | 解锁对话 | Unlock conversation |
| `chat.compact.unlock_confirm` | 解锁后对话可能再次达到上限，是否继续？ | Unlocking may cause the conversation to reach the limit again. Continue? |
| `chat.compact.generate_summary` | 生成交接摘要 | Generate handoff summary |
| `chat.compact.generate_confirm` | 将生成交接摘要并终止当前对话，是否继续？ | This will generate a handoff summary and terminate the conversation. Continue? |
| `chat.compact.error_title` | 摘要生成失败 | Summary generation failed |
| `chat.compact.retry` | 重试 | Retry |
| `chat.compact.conversation_full` | 对话已满，无法发送 | Conversation is full, cannot send |

输入区相关 i18n 键：

| i18n key | zh-CN | en-US |
|----------|-------|-------|
| `chat.input.placeholder_full` | 对话已终止，请生成交接摘要或新建对话 | Conversation terminated — generate a handoff summary or start a new chat |

---

## 7. 前端 UI

### 7.1 对话满横幅

对话满时在输入区上方显示横幅，包含：
- [新建对话] 按钮
- [解锁对话] 按钮（见 §7.6）

摘要内容通过消息流中 `role: compact_summary` 的消息展示（MessageBubble 渲染为交接摘要样式），用户可直接查看和复制。

### 7.2 摘要生成失败横幅

摘要生成失败时显示独立横幅（非对话满横幅），包含：
- 失败原因提示（如"摘要生成超时"/"Provider 返回错误"）
- [重试] 按钮：重新触发生成摘要流程
- [关闭] 按钮：关闭横幅，对话继续正常使用

用户也可通过 §7.3 的 [生成交接摘要] 按钮再次手动触发。

### 7.3 手动触发摘要

对话未满时，`chat-tool-bar`（输入区上方工具栏）提供 [生成交接摘要] 按钮：

- 点击后确认提示："将生成交接摘要并终止当前对话，是否继续？"
- 确认后调用 `manual_compact_conversation` command，走 §3.2 流程
- 流式输出进行中时按钮 disabled（避免与自动触发冲突）

### 7.4 输入区禁用

`is_full` 为 true 的对话，输入框 disabled，placeholder 切换为 `chat.input.placeholder_full`（"对话已终止，请生成交接摘要或新建对话"），`send_chat_message` 返回错误。

### 7.5 侧边栏标记

已满对话标题旁显示锁图标，表示只读。

### 7.6 chatStore 扩展

- 新增 `isFull: boolean` 状态（当前对话是否已满）
- 新增 `compactSummary: string | null` 状态（摘要文本，用于横幅展示）
- 新增 `compactError: string | null` 状态（摘要生成失败原因，null 表示无错误）
- `Conversation` interface 新增 `is_full: boolean` 字段
- 消费 `conversation_terminated` 事件：设置 `isFull=true` + `compactSummary=summary`
- 消费 `conversation_compact_failed` 事件：设置 `compactError=error`
- `list_conversations` 返回的 `is_full` 字段用于侧边栏锁图标
- `switchConversation` 时重置 `isFull` / `compactSummary` / `compactError`

### 7.7 对话解锁

已满对话的横幅中提供 [解锁对话] 按钮，允许用户主动恢复对话可发送状态：

- 点击后确认提示："解锁后对话可能再次达到上限，是否继续？"
- 确认后调用 `unlock_conversation` command，将 `is_full` 重置为 0
- 前端更新 `isFull` 状态，恢复输入区
- 对话解锁后，下次发送消息流结束时仍会正常检查终止条件

适用场景：用户删除了旧消息释放 token、切换了更大上下文窗口的模型、或主动选择忽略限制继续工作。

---

## 8. 架构设计

### 8.1 模块归属

| 模块 | 归属 | 职责 |
|------|------|------|
| `compaction.rs` | assistant-agent | should_terminate + generate_summary + terminating 标记 + DEFAULT_COMPACT_SUMMARY_PROMPT |
| `conversations.rs` | assistant-storage | CompressionStateRow + compression_state CRUD + is_full 读写 |
| `cmd/chat.rs` | assistant-app | 流结束后自动触发终止 + is_full 守卫 + terminating 检查 |
| `cmd/conversation.rs` | assistant-app | manual_compact_conversation + unlock_conversation command |
| `lib.rs` | assistant-app | invoke_handler 注册新 command |
| `fieldDefinitions.ts` | 前端 settings | defineSetting 注册 4 个 compaction 设置项 |
| `GeneralSettings.tsx` | 前端 settings | SettingsGroup 追加 COMPACT_SETTINGS |
| `chatStore.ts` | 前端 stores | isFull + compactSummary + compactError 状态 + 事件消费 |
| `chatTypes.ts` | 前端 chat | Conversation.is_full 字段 |
| `chatService.ts` | 前端 services | manualCompactConversation / unlockConversation invoke |
| `CompactBanner.tsx` | 前端 chat | 对话满横幅（摘要 + 复制 + 新建 + 解锁） |
| `CompactErrorBanner.tsx` | 前端 chat | 摘要失败横幅（错误 + 重试 + 关闭） |
| `ChatView.tsx` | 前端 chat | isFull 条件渲染 CompactBanner / 禁用输入区 / chat-tool-bar（ModeSelector + [生成交接摘要] 按钮） |
| `MessageInput.tsx` | 前端 chat | isFull 时 disabled 输入框 + placeholder_full |
| `ConversationListItem.tsx` | 前端 history | is_full 锁图标 |

### 8.2 数据流

**自动触发路径**：

```
send_chat_message()
  ├─ 检查 is_full 或 terminating → true → 拒绝发送
  ├─ 准备消息、spawn(stream_future)
  └─ stream_future：
       ├─ 流式输出逐字 emit chat-delta
       ├─ finish_reason 到达
       │    ├─ 保存最终内容到 DB
       │    ├─ usage.prompt_tokens 存在？
       │    │    ├─ 是 → should_terminate(prompt_tokens, context_window)
       │    │    │    ├─ true + auto_compact=true → 执行终止流程（见下方）
       │    │    │    └─ false / auto_compact=false：跳过
       │    │    └─ 否 → 跳过
       │    └─ finish_tx.send(conv_id)
       └─ 流结束
```

**手动触发路径**：

```
manual_compact_conversation(conv_id)
  ├─ 检查 is_full 或 terminating → true → 返回错误
  ├─ 检查无正在进行的流 → 有流 → 返回错误
  └─ 执行终止流程（见下方）
```

**终止流程（共享）**：

```
设置 terminating 标记
→ 插入 compact_summary 消息占位（chat-delta 流式推送）
→ generate_summary
  ├─ 成功 → 事务写入 is_full=1 + 摘要 → emit conversation_terminated → 清除 terminating
  └─ 失败 → 清除 terminating，日志记录，对话继续
```

### 8.3 常量（内置默认值）

| 常量 | 值 | 位置 | 用户可覆盖 |
|------|-----|------|-----------|
| `COMPACTION_THRESHOLD_PERCENT` | 80 | Rust constant.rs + TS constants.ts | `compact_threshold_percent` |
| `MAX_SUMMARY_REQUEST_TOKENS` | 4096 | Rust constant.rs | `compact_summary_max_tokens` |
| `MAX_SUMMARY_OUTPUT_TOKENS` | 2000 | Rust constant.rs + TS constants.ts | 不可覆盖（内置截断阈值） |

---

## 9. 参考映射

| 功能点 | 参考项目 | 参考文件 | 借鉴内容 |
|--------|---------|---------|---------|
| 阈值判断 | OpenBitFun | compression_prefetch.rs | `in_prefetch_window` 阈值公式，简化为百分比阈值 |
| 压缩状态 | OpenBitFun | session.rs | `CompressionState` 结构 + `auto_compact` 开关 |
| 摘要策略 | deepseek-harness-orb | compaction-basic | thresholdRatio + retainRatio + 摘要替换最旧跨度 |
| 前缀缓存复用 | deepseek-harness-orb | summarizer.ts | 摘要指令作为最后 user message 追加，复用 KV cache |
| Tool-Result Pruner | deepseek-harness-orb | compaction-tool-result-pruner | 确定性裁剪过大 tool 输出，减少摘要 token |
| 确定性摘要 | airi | compactor.ts | 工具调用保留名称+结果摘要，文本截取片段，[Compacted] 标注 |

---

## 10. 验收标准

1. 长对话 token 达到窗口 80% 时自动终止对话
2. 终止不中断当前流式输出（流结束后异步执行）
3. usage 缺失时不触发终止（避免误终止）
4. 用户可随时手动触发生成交接摘要并终止对话
5. 摘要生成失败时对话继续可用，不标记 is_full，显示失败横幅 + [重试] 按钮
6. 摘要 prompt 支持用户自定义覆盖，None 使用默认模板
7. 摘要生成采用前缀缓存复用策略（重放对话前缀 + 摘要指令作为最后 user message）
8. 摘要为结构化交接文档（目标/进展/待办/关键信息/工具调用摘要），可复制粘贴到新对话
9. 横幅显示 [新建对话] + [解锁对话] 按钮；摘要作为 compact_summary 消息在消息流中可见，用户可直接查看和复制
10. 终止后对话只读：输入区禁用、send_chat_message 拒绝
11. 解锁对话后恢复可发送状态，下次流结束仍检查终止条件
12. 侧边栏已满对话显示锁图标
13. 用户可在设置中关闭自动终止或调整阈值
14. compression_state 表正确持久化压缩状态，摘要作为 compact_summary 消息存入 messages 表
15. is_full 与摘要写入在同一事务中
16. 设置项通过 defineSetting 注册，条件可见（auto_compact 开关控制其余 3 项显隐）
17. i18n 中英文键齐全（settings.general.compact.* + chat.compact.*）
18. cargo check + clippy + test 通过

---

## 11. 延后项

以下功能在 I7 核心完成后视情况接入：

### 11.1 跨对话检索工具

`search_history(query, conversation_id?, limit=5)` — 搜索历史对话消息。

- 依赖 tool-call 执行循环（当前未实现）
- I7 先实现 Tauri command + SQL，tool-call 循环就绪后注册为模型可用工具
- 需考虑搜索结果的敏感信息过滤

### 11.2 Tool-Result Pruner

摘要生成前，确定性裁剪过大的 tool 输出：保留 head/tail，中间替换为 `[pruned]`。减少摘要请求的 token 消耗，不需要 LLM 调用。参考 deepseek-harness-orb compaction-tool-result-pruner。

### 11.3 压缩续接模式

当前为"终止模式"（对话满即结束），未来可支持"续接模式"（摘要替换早期消息，对话继续），参考 deepseek-harness-orb compaction-basic 的 thresholdRatio + retainRatio 策略。

### 11.4 Image Offload

多模态对话中，图片导致上下文超限时自动卸载最旧图片并重试，摘要中图片替换为占位文本。参考 deepseek-harness-orb compaction-image-offload。
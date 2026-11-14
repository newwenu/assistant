# Token Usage 语义契约与展示规范

> 术语与计算式的唯一事实源：`docs/archived/chat/I29-token-usage-semantics-spec.md`（本文不再重复定义公式）
> 现状诊断：`docs/archived/chat/I29-token-usage-analysis.md`
> 本文只定义「字段叫什么、怎么展示、验收判据」，不含实施阶段与任务排期。
> 基线口径：以当前仓库代码为准。

---

## 0 结论速览

| # | 契约 |
| --- | --- |
| 1 | 三个口径必须独立建模：**上下文窗口**（分母）、**上下文占用**（分子）、**累计消耗**（账单）。禁止用累计值填占用位 |
| 2 | **上下文占用的可信来源是「最近一次聊天请求的 `prompt_tokens`」**；仅当该分支零记录时，才允许用分支出站投影估算兜底并标 `~`（只读、不落库）。禁止用历史消息 `tokens` 求和冒充占用 |
| 3 | 持久化单元是**请求级记录**（一次 API 调用一条），而不是把 breakdown 塞进 `messages` 行 |
| 4 | `cacheRead` / `cacheWrite` / `reasoning` 必须区分「未上报」与「上报为 0」，用 `NULL` / `undefined` 表达未上报 |
| 5 | 进度条只画**输入口径的占用**（命中 / 写入 / 未缓存三段，和恒等于 `usedTokens`）；输出、推理、累计消耗一律放 Tooltip / 详情层 |

---

## 1 语义契约

### 1.1 三个口径的权威定义

| 口径 | 字段名 | 定义 | 分母/来源 | 用途 |
| --- | --- | --- | --- | --- |
| 上下文窗口 | `contextWindowTokens` | 模型单次请求可容纳的输入+输出上限 | 模型配置 `models_json.contextWindow`，本地注入 | 进度条分母、压缩阈值基准 |
| 上下文占用 | `usedTokens` | 下一次请求将发送的输入 token 数，以「最近一次 `kind = chat` 请求的 `prompt_tokens`」为代理值（两者构成相同）；该分支零记录时退回分支出站投影估算（标 `~`，见 §3.4） | 最近一次 `kind = chat` 请求的 `prompt_tokens`；零记录时为投影估算 | 进度条分子、`percentage` |
| 累计消耗 | `cumulativeTokens` | 本会话所有请求 `prompt + completion` 之和 | 请求记录表聚合 | Tooltip / 详情，计费参考 |

`percentage = usedTokens / contextWindowTokens`，clamp 到 `[0, 1]`。

**占用口径选定：输入口径。** `usedTokens` 只含输入，不含本次 `completion`。

理由三条：

1. 进度条回答的是「本次请求发送了多少、下一次请求还剩多少额度」，而 `completion` 是本轮输出、不属于本轮输入——它生成的文本会在下一次请求时作为历史重新发送，已经计入新的 `P`；
2. 流式期间本仓库只能拿到 `promptTokens`，若改用含输出口径，同一字段在「流式」与「切会话」后又会分裂成两种语义（即 A1 重演）；
3. 与 openhuman 的 `last_turn_input_tokens`、deepseek-harness-orb 的 `pressureTokens` 同口径，便于横向对照。

含输出口径（codex 的 `last_token_usage.total_tokens`）是另一种合法口径，本文不采用。两种口径相差一个 `completion`，差异与取舍见 `I29-token-usage-semantics-spec.md` §5。输出 token 只通过 Tooltip 展示，不进入进度条。

### 1.2 字段级不变量

跨 provider 恒成立，任何一层实现都不得违反：

1. `cacheRead ⊆ prompt`
2. `cacheWrite ⊆ prompt`（仅 Anthropic 系有值）
3. `reasoning ⊆ completion`
4. `uncachedInput = prompt - cacheRead - cacheWrite`，下限 0
5. `total = prompt + completion`
6. `usedTokens` 在一次请求内是快照、不变；跨请求可增可减（分支切换、编辑重发都会改变历史长度），因此不得假设它单调递增

### 1.3 「未上报」与「零」必须可区分

| 语义 | Rust | TS | 展示 |
| --- | --- | --- | --- |
| provider 未上报该字段 | `None` | `undefined` | 不显示该项 |
| provider 明确上报 0 | `Some(0)` | `0` | 显示 `0` |

这条对 `cacheRead` / `cacheWrite` / `reasoning` 三项强制生效。缓存命中率的分母只统计「至少上报一项缓存字段」的请求，否则未支持缓存的 provider 会稀释命中率；判定谓词见 §3.3。

### 1.4 命名规则（消除歧义）

完整禁令表（含文案类）见 `I29-token-usage-semantics-spec.md` §7；下表只列**字段命名**层面的强制项，因为它们直接决定接口契约。禁用以下命名，因为它们在多上下文中含义漂移：

| 禁用 | 原因 | 应改为 |
| --- | --- | --- |
| `totalTokens` 单独出现 | 既可指本请求 total，也可指会话累计 | `requestTotalTokens` / `cumulativeTokens` |
| `usedTokens` 指累计值 | 与「占用」语义冲突 | `cumulativeTokens` |
| 「上下文窗口总 token 花费」 | 窗口与花费是两个量，字面矛盾 | 「窗口占用」或「累计消耗」 |
| `tokens`（裸字段，指整轮 prompt+completion） | 易被误读为「这条消息自己的 token」 | `requestTotalTokens`，并在文档标注其为整轮口径 |

---

## 2 现状基线

本节编号（P / W / Q / F）与 `I29-token-usage-analysis.md` 的 A1–A20 描述的是同一批代码事实的两组切面，映射表见 `I29-token-usage-semantics-spec.md` §8.1。同一事实两处出现时以本节的结构性事实为准，现象描述以 analysis 为准。

### 2.1 持久化层：只存了 1 个数字

| # | 事实 | 位置 |
| --- | --- | --- |
| P1 | `messages.tokens` 有 usage 时存整轮 `prompt + completion`；无 usage 时只存回复文本的字符估算（`resolve_final_tokens` 仅对 `full_content` 估算） | `assistant-storage/src/db.rs`、`assistant-agent/src/chat_runtime.rs` |
| P2 | `MessageRow.tokens: Option<i64>` 仅一个字段 | `assistant-storage/src/conversations/mod.rs` |
| P3 | `update_message_content()` 只能更新这 1 个 `tokens` | `assistant-storage/src/conversations/msg_ops.rs` |
| P4 | `MSG_SELECT` 无任何 breakdown 列 | 同上 |

### 2.2 写入路径：breakdown 在运行时丢失

| # | 事实 | 位置 |
| --- | --- | --- |
| W1 | `StreamOutcome` 只有 `total_tokens` / `prompt_tokens` 两个 token 字段 | `assistant-agent/src/chat_runtime.rs` |
| W2 | `StreamAccumulator` 收到 `usage` 时只提取这两个字段，其余字段丢弃 | 同上 |
| W3 | 累加器自行计算 `total = prompt + completion`，覆盖 provider 的 `total_tokens` | 同上 |
| W4 | `prompt_tokens` 只用于压缩判定，判定后丢弃 | `src-tauri/src/cmd/chat/mod.rs` |
| W5 | 压缩请求的 usage 完全丢弃，摘要消息写 `tokens: None` | `src-tauri/src/cmd/compaction.rs` |
| W6 | `should_terminate` 在窗口为 `0` 时直接返回 `false`，未知窗口不触发压缩 | `assistant-agent/src/compaction.rs` |
| W7 | 超窗错误 `context_length_exceeded` 仅出现在测试夹具里，没有匹配分支与降级动作 | `assistant-providers/src/client/error.rs` |
| W8 | `From<ChatUsage>` 把 `reasoning_token_count` / `cache_creation_token_count` 硬编码为 `None`，`ChatUsage` 未建模 `completion_tokens_details` | `assistant-providers/src/openai/{types.rs,stream.rs}` |

### 2.3 聚合查询：SUM 混用两种口径

| # | 事实 | 位置 |
| --- | --- | --- |
| Q1 | `TokenUsageSummary` 仅 `total_tokens` / `context_window_tokens` / `percentage` | `assistant-storage/src/agent_modes.rs` |
| Q2 | SQL 为 `SUM(COALESCE(tokens, (LENGTH(content)+3)/4+4))`，把上报值与估算值相加 | 同上 |
| Q3 | 该 SUM 结果被当作「上下文占用」返回 | 同上 |
| Q4 | `TokenUsageSummaryDto` 仅 3 字段，且 `deny_unknown_fields` | `src-tauri/src/protocol/dto/chat.rs` |
| Q5 | `is_full` 只由压缩成功路径置 `true`、由 `unlock_conversation` 置 `false`，与窗口占用无关 | `assistant-storage/src/conversations/compression.rs`、`src-tauri/src/cmd/compaction.rs` |
| Q6 | 消息层按 `active_branch_id` 追踪（`trace_active_branch`），token 聚合无分支维度 | `assistant-storage/src/conversations/{mod.rs,branch.rs}` |

### 2.4 前端：全量数据已到，只消费 1 个字段

| # | 事实 | 位置 |
| --- | --- | --- |
| F1 | `TokenUsageDto` 已含 7 字段（prompt / completion / total / cache_read / cache_write / reasoning / context_window） | `src-tauri/src/protocol/dto/chat.rs` |
| F2 | 流式 delta 携带完整 `usage` | 同上 |
| F3 | `chatOrchestrator` 只取 `promptTokens` → `ContextUsage.usedTokens` | `src/services/chatOrchestrator.ts` |
| F4 | `ContextUsage` 仅 3 字段 | `src/chat/core/chatTypes.ts` |
| F5 | `finishStream` 用 `promptTokens`，`fetchTokenUsage` 用 `summary.totalTokens`，写入同一字段 | `src/stores/chatStorePorts.ts`、`src/chat/core/chatUtils.ts` |
| F6 | `ContextBar` 只显示 `used / max` + 进度条 + 百分比 | `src/chat/ChatView.tsx` |
| F7 | 阈值常量 70% / 90% | `src/config/constants.ts` |
| F8 | `cacheReadTokens` / `cacheWriteTokens` / `reasoningTokens` 仅有类型声明，`src/` 内无任何读取点 | `src/chat/core/chatTypes.ts` |

---

## 3 数据模型规范

### 3.1 选型：请求级记录表

| 方案 | 优点 | 致命问题 |
| --- | --- | --- |
| A. 扩展 `messages` 加 5 列 | 改动面小，与消息天然共处 | ① 占用仍需猜（最后一条消息的 `tokens` 是整轮 total，含 completion，会高估）；② `update_message_content` 参数从 6 增至 11，触发 clippy `too_many_arguments` 门禁；③ 重试 / 压缩请求无处落；④ 存量行全 NULL，聚合需大量 `COALESCE` |
| B. 独立 `token_usage_records` 表 | ① 一次请求一条，天然支持重试与压缩；② 占用 = 最新记录 `prompt_tokens`，精确；③ 不污染 `messages`；④ 可按 model / 时间窗聚合 | 多一张表 + 一次 JOIN 级写入 |

**选定 B。** 判据是语义正确性：只有请求级记录能同时满足「占用 = 最近一次请求的 prompt」与「累计 = 所有请求之和」，而这两者在方案 A 中必须共用同一个标量。

这不是偏好问题，四个参考项目各自用不同存储形态给出了同一结论：

| 参考项目 | 请求级事实的载体 | 占用与累计如何分离 |
| --- | --- | --- |
| OpenBitFun | `TokenUsageRecord`（一次 `record_usage` 一条，带 `turn_id`） | 另有 `SessionContextUsage` 投影，`source` 区分 `ModelRequest` / `ContextCompression` |
| deepseek-harness-orb | 事件日志（`assistant/message` 的 usage 样本） | `tokenUsage` 与 `contextPressure` 两个投影互不替换 |
| codex | `TokenUsageRecord`（请求级 / 轮级 / 线程级三层同时落） | `last_token_usage` 作仪表分子，`total_token_usage` 只作账单 |
| openhuman | transcript 逐条 `usage` | `last_turn_input_tokens` 单列为仪表分子，累计字段另计 |

四者都**没有**把 breakdown 塞进消息行，也都没有用累计值填占用位。

### 3.2 表结构

```sql
CREATE TABLE IF NOT EXISTS token_usage_records (
  id                    INTEGER PRIMARY KEY AUTOINCREMENT,
  conversation_id       TEXT    NOT NULL,
  message_id            TEXT,
  attempt_id            TEXT,
  branch_leaf_id        TEXT,              -- 该请求发出时的活跃分支叶子；NULL = 会话未启用分支
  kind                  TEXT    NOT NULL,   -- 'chat' | 'compaction'
  provider_id           TEXT,
  model_id              TEXT,
  prompt_tokens         INTEGER NOT NULL,
  completion_tokens     INTEGER NOT NULL,
  cache_read_tokens     INTEGER,           -- NULL = 未上报
  cache_write_tokens    INTEGER,           -- NULL = 未上报
  reasoning_tokens      INTEGER,           -- NULL = 未上报
  cache_capable         INTEGER NOT NULL DEFAULT 0,  -- 1 = 该 provider/model 具备缓存能力，由适配层声明
  context_window_tokens INTEGER,           -- NULL = 该请求未注入窗口，不得用 0 替代
  estimated             INTEGER NOT NULL DEFAULT 0,  -- 1 = 含估算值
  created_at            INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_token_usage_conv
  ON token_usage_records (conversation_id, branch_leaf_id, created_at DESC, id DESC);
```

字段约束：

| 字段 | 约束 |
| --- | --- |
| `kind` | 决定是否参与占用计算；`compaction` 只参与累计 |
| `prompt_tokens` / `completion_tokens` | 非空；provider 未上报时按 §3.4 回退，并置 `estimated = 1` |
| `cache_read_tokens` 等三项 | 可空，`NULL` 即「未上报」，不得用 0 替代 |
| `cache_capable` | 适配层声明的缓存能力（无缓存 / 读缓存 / 读写缓存）。命中率分母要求它为 1，不能靠 `cache_read_tokens = 0` 反推（M14） |
| `attempt_id` | 重试时区分同一轮内的多次请求；每次成功或失败的带 usage 请求都落一条 |
| `branch_leaf_id` | 写入该请求发出时的 `active_branch_id`（无分支时为 `NULL`）。占用查询按它过滤，否则切分支后锚点会指向已弃用的历史（A14） |
| `message_id` | 关联 assistant 消息；压缩请求指向摘要消息 |
| `context_window_tokens` | 可空。`NULL` = 该请求发送时未注入窗口（如压缩路径），与 `0` 严格区分；`0` 不是合法窗口值，写入时归一化为 `NULL` |
| `estimated` | 任一 token 字段走了估算即为 1；UI 对估算值加 `~` 前缀 |

### 3.3 派生视图（三个口径各一条查询）

```sql
-- 上下文占用（第一步）：当前分支上最新一条「非估算」chat 请求
-- ?2 = 当前 active_branch_id（无分支时为 NULL）
SELECT prompt_tokens, context_window_tokens, estimated
FROM token_usage_records
WHERE conversation_id = ?1
  AND kind = 'chat'
  AND branch_leaf_id IS ?2
  AND estimated = 0
ORDER BY created_at DESC, id DESC
LIMIT 1;
-- 上一步无结果时，去掉 `estimated = 0` 重跑同一条查询，取最近一条估算记录（UI 加 `~`）
-- 前两步都无结果时，由 app 层按分支出站投影估算兜底（第三步：不查表、不落库，见 §3.4 第 4 级）

-- 累计消耗：所有请求之和（含压缩与重试，不按分支过滤）
-- 必须同时取 COUNT(*)：零记录时返回 NULL / None（UI 显示「—」），不得用 0 冒充「没花过」（M4 / M16）
SELECT SUM(prompt_tokens + completion_tokens) AS total, COUNT(*) AS n
FROM token_usage_records WHERE conversation_id = ?1;

-- 缓存命中率：分母只含「具备缓存能力且至少上报了一项缓存字段」的请求
SELECT COALESCE(SUM(COALESCE(cache_read_tokens, 0)), 0) AS hit,
       COALESCE(SUM(prompt_tokens), 0)                  AS denom
FROM token_usage_records
WHERE conversation_id = ?1
  AND cache_capable = 1
  AND (cache_read_tokens IS NOT NULL OR cache_write_tokens IS NOT NULL);
```

> 命中率 = `hit / denom`；`denom = 0` 时返回 `None`（不显示），而不是 `0`。
>
> 分母谓词必须是「任一缓存字段非 `NULL`」，不能只判 `cache_read_tokens`：Anthropic 首次请求只报写缓存、不报读缓存，只判读字段会把它错误排除，命中率恒为 0。该谓词还会把「provider 不支持缓存但恒报 `0`」的请求计入分母、稀释命中率（M14），因此分母叠加 `cache_capable = 1`：能力由适配层声明并随记录落库，不能只靠「是否上报」或 `0` 值反推。

三条与排序和分支相关的约束：

1. **占用必须按分支过滤，累计不得按分支过滤。** 占用回答「即将发送多少」，只能看当前分支；累计是账单，切分支不退款（A14）。`branch_leaf_id IS ?2` 在 `?2` 为 `NULL` 时退化为「无分支的请求」，不会误纳其他分支的记录。
2. **排序必须有二级键。** `created_at` 是毫秒，重试的两次请求可能同毫秒；只按 `created_at DESC` 时「最新一条」不确定（A19），因此补 `id DESC`（自增主键即写入序）。
3. **估算记录的优先级低于上报记录。** 查询优先取 `estimated = 0` 的记录；仅当该分支上不存在任何非估算记录时，才退回最近一条估算记录，并由 UI 加 `~` 前缀（M3）。实现上等价于「先查 `estimated = 0`，无结果再查全量」，不能用一条 `LIMIT 1` 混查。两类都取不到时，才进入 §3.4 第 4 级的投影兜底。

### 3.4 回退顺序

`prompt_tokens` 的取值优先级，用于「provider 未上报 usage」的模型：

1. provider 上报的 `prompt_tokens`
2. `total_tokens - completion_tokens`（若 total 有值）
3. 字符估算（E1，`assistant-agent/src/token_estimate.rs` 的 `estimate_text_tokens`）：**不是**记录表的 `prompt_tokens` 来源（记录回退只走 1–2）；它只作为决策档（offload 预算）与第 4 级投影的文本分量口径
4. **分支出站投影估算（无任何记录时）**：该分支没有任何 `chat` 记录时，取当前分支经请求构造投影后的 `chat_messages`（`prepare_messages` 的 system prompt + 历史消息，再经图片 / 文件投影），按 E2 的 `estimate_total_tokens` 求和，作为占用兜底。该口径与 offload 预算共用；工具定义当前不随请求发送，故不在该口径内（若未来启用工具，需同步纳入，见 analysis §8）
5. **分支无消息** → 占用确定为 `0`（无历史输入可计），窗口已知时展示 `0 / 窗口（0%）`，窗口未知时展示「—」（`hasUsage = false`）

1–2 用于**落库**（记录表字段回退；回退得到的值不是上报值，必须置 `estimated = 1`）；3 是 2 与 4 共用的一套 E1/E2 口径，本身不单独产生记录；4 用于**展示兜底**，只读不写——**不写入 `token_usage_records`**、**不写入 `messages.tokens`**、**不进入累计 `A`**，每次查询重算。三者共用同一套估算口径，但只有 1–2 能进入记录表。

非 ASCII 系数取中文实测密度的下限（1~1.5 token/字），因此即使按契约目标值仍属**低估**；该偏差是单向的，对 offload 预算会导致漏判、对展示兜底会导致百分比偏低。取值依据与各参考项目的对照见 `I29-token-usage-semantics-spec.md` §3.5。

估算值必须可辨识：落库时置 `estimated = 1`，展示时加 `~` 前缀并标明来源（「本地估算」），累计 `A` 只统计上报值、不含任何估算。

---

## 4 各层结构规范

### 4.1 Rust 运行时

| 结构 | 契约 |
| --- | --- |
| `StreamOutcome` | 必须完整携带 `prompt_tokens` / `completion_tokens` / `cache_read_tokens` / `cache_write_tokens` / `reasoning_tokens` / `context_window_tokens`，不得只留 total |
| `PartialOutcome` | 同上；流中断时的部分 usage 同样要落记录 |
| `StreamAccumulator` | 每收到一次 `usage` 就整体替换为本轮最新值（usage 是累计快照，不是增量）；`total` 优先取 provider 的 `total_tokens`，缺失时用 `prompt + completion`；两者冲突时按 M12 裁决：`total < P + C` 视为 provider 漏报，回退 `P + C` 并置 `estimated` |
| provider 适配层 | 每个 provider 的流式解析必须填满 5 个 usage 字段，确实没有的填 `None`，不得硬编码 `None` 掩盖可解析字段（A15：`ChatUsage` 需建模 `completion_tokens_details.reasoning_tokens`，`From<ChatUsage>` 再映射到 `reasoning_token_count`） |
| 压缩路径 | 压缩请求的 usage 同样落一条 `kind = 'compaction'` 记录，摘要消息的 `tokens` 保持 `NULL`（它是摘要，不是一轮对话） |

### 4.2 DTO

| 结构 | 契约 |
| --- | --- |
| `TokenUsageDto` | 既有 7 字段不改，仅 `context_window_tokens` 由 `u32` 改为 `Option<u32>`：未知窗口必须能序列化为缺省字段，不能退化为 `0`（A13）。新增字段时注意 `deny_unknown_fields` 对前后端同步的强约束 |
| `TokenUsageSummaryDto` | 拆成三个语义独立的结构或字段组：`occupancy`（used / window / percentage）、`cumulative`（total / prompt / completion）、`cache`（hit / denom / ratio），不得再用单个 `total_tokens` 兼顾两者 |

### 4.3 TypeScript

| 结构 | 契约 |
| --- | --- |
| `ContextUsage` | 字段定义见下表；`usedTokens` 恒为**占用**，`maxTokens` 恒为窗口 |
| `cumulativeTokens` | 独立字段，不得复用 `usedTokens` |
| 写入点 | 流式 `usage` → 更新占用与 breakdown；流结束 → 更新累计；切换会话 → 从记录表取占用与累计；三者不得互相覆盖语义 |

`ContextUsage` 字段定义（`src/chat/core/chatTypes.ts`）：

| 字段 | 类型 | 语义 | 可空 |
| --- | --- | --- | --- |
| `usedTokens` | `number` | 占用 `O`，输入口径 = 最新 chat 请求的 `P`。它同时就是 Tooltip 的「本次输入」，因此不再另设 `promptTokens` 字段 | 否 |
| `hasUsage` | `boolean` | 占用是否有可用取值。`true` 表示存在上报值、估算记录或分支投影估算；`false` 仅当该分支无消息（占用确定为 `0`）。窗口已知时 `false` 仍展示 `0 / 窗口（0%）`，窗口未知时才显示「—」，`usedTokens` 不参与百分比计算。不变式：`estimated = true` 时 `hasUsage` 必为 `true` | 否 |
| `estimated` | `boolean` | 该 `usedTokens` 是否为估算。`true` 有两种来源：记录表里 `estimated = 1` 的请求（provider 未上报输入 token，由 `total - completion` 回退），或该分支零记录时的出站投影。两种来源都必须加 `~` 并标明来源 | 否 |
| `estimatedSource` | `'record' \| 'projection' \| undefined` | 估算来源，仅在 `estimated = true` 时有值：`record` = 记录表估算记录；`projection` = 分支出站投影。用于区分 Tooltip 的来源文案（前者是「未上报输入 token」，后者是「未上报 usage」） | 是 |
| `maxTokens` | `number \| undefined` | 窗口 `W`；窗口未知时不得填缺省值，置 `undefined` | 是 |
| `percentage` | `number \| undefined` | `clamp(usedTokens / maxTokens, 0, 1)`；`maxTokens` 为 `undefined` 时同为 `undefined` | 是 |
| `completionTokens` | `number` | 最近一次请求的输出 `C`，仅 Tooltip 使用 | 是 |
| `cacheReadTokens` | `number` | `CR` | 是，未上报为 `undefined` |
| `cacheWriteTokens` | `number` | `CW` | 是，未上报为 `undefined` |
| `reasoningTokens` | `number` | `RT` | 是，未上报为 `undefined` |
| `cumulativeTokens` | `number` | 累计 `A`；该会话零记录时为 `undefined`（显示「—」），不得用 `0` 冒充「没花过」 | 是 |

### 4.4 窗口解析

`contextWindowTokens` 的解析顺序（只在后端执行一次，前端不重复推导）：

1. 会话所用模型的 `models_json.contextWindow`（用户配置，优先级最高）
2. provider 发现接口返回的 `context_length`（`api_context`）
3. 内置模型目录（`model_catalog`，含按模型名的已知常量）
4. 以上都取不到 → **未知**，返回 `undefined` / `NULL`，不得回退到硬编码常量

前端不得再用 `providerStore.models[].contextWindow` 自行推导窗口：视图与请求构造各自解析会让同一会话出现两个分母（流式 usage 用 `api_context`、切会话查询用 `models_json`）。`get_conversation_token_usage` 未显式传窗口时由后端按上表解析并返回 `context_window_tokens`（可空），前端只消费该返回值。

硬编码的 `128_000` 只能用作设置页滑块的取值范围，不能用作「窗口未知」时的兜底值——那会让未知模型永远显示 128k，并让压缩阈值按错误的窗口计算（A6）。

两条配套规则：

1. **未知覆盖已知。** 新上下文未声明窗口时，必须清空 `maxTokens`，不得沿用上一次已知值。沿用会让切模型后长期按旧分母算比例（M11）。来源：deepseek-harness-orb 的 `contextPressure` fold 在新事件不带窗口时主动删除旧值，并有测试锁定该行为。
2. **百分比与绝对数互斥。** `percentage` 存在时显示百分比；不存在时显示绝对 token 数。两者不得同时出现——同时出现会让用户以为「100% 的窗口只用了 12,345 token」。来源：codex 的 `context_used_tokens(info, percent_known)` 在百分比已知时返回 `None`，正是这条互斥规则。

---

## 5 UI 展示规范

### 5.1 分层展示

| 层 | 内容 | 数据源 |
| --- | --- | --- |
| 常驻行（ContextBar） | `占用 / 窗口` + 进度条 + 百分比 | `usedTokens` / `maxTokens` |
| 常驻行（占用为估算） | `~占用 / 窗口` + 进度条 + 百分比；`~` 只标在被估算的分子上，百分比不重复加 `~` | `usedTokens` + `estimated` |
| 常驻行（窗口未知） | 只显示 `占用` 绝对 token 数（估算时同样带 `~`），不画进度条、不显示百分比 | `usedTokens` / `estimated` |
| 常驻行（无内容） | 窗口已知：`0 / 窗口` + 空进度条 + `0%`；窗口未知：「—」 | `hasUsage = false` + `maxTokens` |
| Tooltip（hover） | 本次输入 / 输出 / 缓存命中 / 缓存写入 / 推理 + 累计消耗 + 占用来源 | `usedTokens` / `completionTokens` / `cacheReadTokens` / `cacheWriteTokens` / `reasoningTokens` / `cumulativeTokens` / `estimated` |
| 详情面板（可选展开） | 按请求逐条记录、按 model 聚合 | 记录表查询 |

进度条只表达占用，不表达累计。累计值不得画进同一根进度条。

占用来源行是估算值的配套要求，按 `estimated` / `estimatedSource` / `hasUsage` 三选一：

| `estimated` | `estimatedSource` | `hasUsage` | 来源行文案 |
| --- | --- | --- | --- |
| `false` | `undefined` | `true` | 「provider 上报」 |
| `true` | `record` | `true` | 「本地估算（provider 未上报输入 token）」 |
| `true` | `projection` | `true` | 「本地估算（provider 未上报 usage）」 |
| `false` | `undefined` | `false` | 「尚无消息」（占用 `0` 的确定值，不是估算） |

累计为零记录时累计行显示「—」，不显示 `0`。

### 5.2 进度条分段

进度条只画输入，三段宽度之和恒等于 `usedTokens`，与 §1.1 的占用口径严格一致：

| 段 | 宽度 | 颜色 | 显示条件 |
| --- | --- | --- | --- |
| 缓存命中输入 | `cacheReadTokens / maxTokens` | 绿 | `cacheReadTokens > 0` |
| 缓存写入输入 | `cacheWriteTokens / maxTokens` | 青 | `cacheWriteTokens > 0` |
| 未缓存输入 | `(usedTokens - cacheReadTokens - cacheWriteTokens) / maxTokens` | 蓝 | 恒显示 |

必须保留「缓存写入」这一段：`cacheWrite ⊆ usedTokens` 但既不命中也不未缓存，只画命中与未命中两段会让三段之和少一个 `cacheWrite`，条长与 `usedTokens` 对不上。

约束：当 `cacheReadTokens + cacheWriteTokens > usedTokens`（provider 上报矛盾）时，未缓存段按 0 处理，命中段与写入段按比例截断，保证三段之和不超过 `usedTokens`。

输出与推理不进进度条，只放在 Tooltip 单独一行。原因：`completion` 不属于本轮输入（它生成的文本已作为历史计入下一次请求的 `P`），若画进容量进度条，条长会比 `usedTokens` 多一个 `completion`，与百分比读数对不上。codex 采用含输出口径（`last_token_usage.total_tokens`）属另一种合法选择，本文不采用，理由见 §1.1。

### 5.3 阈值与状态

沿用现有三档，语义不变：

| 区间 | 状态 | 颜色 |
| --- | --- | --- |
| `< 70%` | normal | 主题色 / 分段色 |
| `70% ~ 90%` | warning | 黄 |
| `>= 90%` | danger | 红 |

阈值判定只用 `percentage`（占用口径），与累计值无关。

三条配套规则：

1. **窗口未知时三档不生效，也不许用兜底窗口补出判定。** `percentage` 为 `undefined` 时状态恒为 normal（只有绝对数可展示），压缩与降级判定同样返回「不可判定」，不得把「不判定」当成「不超限」而静默跳过（A16 / M13）。此路径下的唯一处置来源是 provider 的超窗错误反馈（见 §6）。
2. **`threshold_percent` 的含义随公式改变，必须同步改名。** 当前实现是「占**完整窗口**的百分比」（`P >= W * 80%`，`assistant-config/src/constant.rs` 的 `COMPACTION_THRESHOLD_PERCENT = 80`，前端 `config/constants.ts` 同值）。改造后阈值改按**可用预算**（`W - max_tokens - 安全裕度`）计算，同名配置项的数值含义随之漂移（M15 / C2），因此必须同步改名（如 `compact_threshold_percent_of_budget`）或直接改为绝对 token 预算；不得保留旧名承载新语义。
3. **估算占用同样参与三档着色，但数值必须带 `~`。** 估算百分比只用于视觉提示，不触发自动压缩（压缩只用 provider 上报的 `P`）。这样「provider 不报 usage」的会话仍能看到黄 / 红提示，而不是在快满时毫无反馈。

### 5.4 文案规范

| 场景 | 正确文案 | 禁止文案 |
| --- | --- | --- |
| 进度条 | 「上下文占用 12.3k / 128k（9.6%）」 | 「上下文窗口总 token 花费」 |
| 进度条（估算） | 「上下文占用 ~12.3k / 128k（9.6%）」 | 「上下文占用 12.3k / 128k」（缺 `~`，冒充上报值） |
| Tooltip 占用来源行 | 「provider 上报」/「本地估算（provider 未上报输入 token）」/「本地估算（provider 未上报 usage）」 | 只写「约 12.3k」而不说明来源；两种估算混用同一句文案 |
| Tooltip 累计行 | 「本会话累计消耗 45.6k」 | 「已用 45.6k」（与占用混淆） |
| Tooltip 累计行（零记录） | 「本会话累计消耗 —」 | 「本会话累计消耗 0」（把「未上报」说成「没花过」） |
| 压缩完成 | 「已存档，会话已终止」 | 「已压缩，上下文已释放」 |

压缩后原会话已 `is_full` 锁定、不再发送，占用不下降，文案不得暗示释放（分析文档 §3.4、§3.5）。

> **暂不生效（续接未纳入计划）**：若未来实现续接，才存在第二种语境——「已由摘要接管历史」。当前仓库只存在「原会话」语境，上表即完整文案契约。

---

## 6 边界与降级

| 场景 | 行为 |
| --- | --- |
| 会话无任何记录，但分支有消息（存量数据 / provider 不报 usage） | 按分支出站投影估算显示 `~N`（`estimated = true`），**不回填记录表、不写 `messages.tokens`**（M16 / A20）。有上报值时一律用上报值，估算不得覆盖上报值（M3） |
| 会话无任何记录，且分支无消息（新会话） | 占用确定为 `0`：窗口已知时显示 `0 / 窗口（0%）`（空进度条、`hasUsage = false`），窗口未知时显示「—」；累计仍显示「—」 |
| 窗口未知 | `maxTokens` / `percentage` 置 `undefined`，占用显示为绝对 token 数、不画进度条；不得用 128k 冒充分母（见 §4.4）。切换模型 / 会话后若新上下文未声明窗口，必须清空旧窗口而非沿用（M11） |
| provider 不报 usage | 占用按分支投影估算显示 `~N`，Tooltip 标明「本地估算」；`messages.tokens` 与记录表保持空，累计显示「—」；压缩判定仍为「不可判定」，唯一处置来源是 provider 超窗错误（M13 / A20） |
| 估算值随消息变化 | 每次查询重算，不做缓存；同一分支新增 / 删除消息后占用随之变化，不残留上一次的估算值 |
| `cacheReadTokens` 未上报 | 不画命中段，Tooltip 不显示命中行；未缓存段宽度改为 `usedTokens - cacheWriteTokens` |
| `cacheWriteTokens` 未上报 | 不画写入段，未缓存段宽度改为 `usedTokens - cacheReadTokens` |
| `reasoningTokens` 未上报 | Tooltip 不显示推理行 |
| 流式中（usage 尚未到达） | 保持上一次占用值，不归零 |
| 发送瞬间 | 不清空 `usedTokens`，只把状态切到 sending |
| 切换会话 | 先按记录表取占用；取不到时按分支投影估算显示 `~N`；分支无消息时窗口已知显示 `0 / 窗口`、窗口未知显示「—」，不得先写缺省窗口再异步覆盖 |
| 压缩完成后 | 占用锚点不重置（会话已锁定、不再发送），仅 `is_full` 置位 |
| 续接会话（暂不生效） | 续接未纳入计划，当前不会出现。若未来实现：首次请求前占用显示「—」（不得用摘要估算值充当占用），首次请求后按其 `prompt_tokens` 锚定，不沿用原会话的 `O`（M6） |
| provider 报超窗错误（`context_length_exceeded` 等） | 捕获该错误并给出可操作提示（「上下文超限，请先压缩 / 新建对话 / 换更大窗口模型」）；窗口已知时不自动重试、窗口未知时不猜测窗口大小，不得静默当作普通失败（A17，T5.6 已落地） |
| `is_full` | 只表示「已压缩锁定、不再发送」，与窗口占用无关（A18）。命名与文案不得使用容量措辞；`unlock_conversation` 解锁后回到压缩前状态，占用锚点随之回退到该分支最新一条 chat 记录 |
| 同一毫秒多条记录 | 占用取 `created_at DESC, id DESC` 的第一条，`id` 作为写入序的稳定二级键（A19） |
| 已存在更晚的估算记录 | 占用按四级来源取值：最近的**非估算** chat 记录 → 最近的估算记录（标 `~`）→ 分支投影估算（标 `~`）→ 无内容（显示「—」）。前一级有结果时不得使用后一级（M3） |
| 累计为零记录 | 累计显示「—」（`cumulativeTokens = undefined`），不得显示 `0`（M4 / M16） |
| 重试 attempt | 每次带 usage 的请求都落记录；占用取最新一条，累计全量相加 |

---

## 7 验收判据

语义层：

1. 同一会话在「流式结束」与「切走再切回」后，`usedTokens` 相同（来自同一条 `prompt_tokens`），不存在两种口径。
2. 128k 窗口、10 轮对话（首轮 `S = 2k`、每轮新增输入 `d = 1k`、每轮输出 `c = 1k`，即每轮净增 2k）的会话：占用 `O = S + 9(d+c) = 20k`（15.6%），累计 `A = 10S + 45(d+c) + 10c = 120k`；两者在 UI 上分处两层，进度条只用前者。推导见 `I29-token-usage-semantics-spec.md` §3.2。
3. `cacheReadTokens` / `cacheWriteTokens` / `reasoningTokens` 未上报时，TS 侧为 `undefined` 而非 `0`。
4. 缓存命中率的分母只含「至少上报一项缓存字段」的请求（§3.3 谓词）。

数据层：

5. 一次成功聊天产生 1 条 `kind = 'chat'` 记录；一次重试产生 2 条（同一 `attempt_id` 前缀不同）；一次压缩产生 1 条 `kind = 'compaction'` 记录。
6. 压缩请求的 token 计入累计，不计入占用。
7. `messages.tokens` 与记录表中的对应请求 total 一致（若保留该列）；无 usage 时该列为 `NULL` 而非估算值。
8. 存量数据库升级后新表为空：有消息的旧会话按分支投影估算显示 `~N`（`estimated = true`）；空会话窗口已知时显示 `0 / 窗口`（`hasUsage = false`），窗口未知时显示「—」；不报错、不回填记录表。

展示层：

9. `completionTokens` / `cacheReadTokens` / `reasoningTokens` 全部缺失时，进度条退化为单色，布局不变。
10. 占用下降（如分支切换）后进度条同步下降，不残留旧值。
11. 窗口未知（模型未配置 `contextWindow`）时，`maxTokens` / `percentage` 为 `undefined`，常驻行只显示绝对 token 数，不出现进度条与百分比；切换到未知窗口的模型后，不沿用上一个模型的窗口值。
12. 切到旧分支后，占用显示该分支最新一条 chat 请求的 `P`，不残留其他分支的值；累计值不随分支切换而下降。
13. 同一毫秒内两次 attempt 各落一条记录时，占用取 `id` 更大的那条（写入序稳定，不依赖时间戳精度）。
14. 窗口未知时不触发自动压缩与降级（判定为「不可判定」）；provider 返回超窗错误时给出可操作提示，而不是静默失败。
15. `threshold_percent` 改名或改为绝对预算后，旧配置文件能被迁移或明确报错，不出现「同名不同义」的静默漂移。
16. provider 完全不报 usage 时，占用显示 `~N` 且 Tooltip 来源为「本地估算」；`messages.tokens` 仍为 `NULL`、记录表仍为空、累计显示「—」。
17. 上报值与估算值共存时，占用取上报值且不带 `~`；估算值不得覆盖上报值。
18. 同一分支新增 / 删除消息后，未落记录的占用随之变化（每次查询重算，不残留上一次的估算值）。

---

## 8 不做什么

- 不把 breakdown 塞进 `messages` 表（见 §3.1 判据）
- 不实现货币计价与成本估算
- 不实现「预测占用」（按当前编辑中的输入预估下一轮）——deepseek-harness-orb 用 `projectedTokens = pressureTokens + surfaceTokens - sampledSurfaceTokens` 提供该能力，OpenBitFun 用 `TokenAnchor` 的尾部估算等价实现；两者都依赖事件级的 surface 变更记录，本仓库无此基础设施，故不纳入本文契约
- §3.4 第 4 级的「分支投影估算」不属于上述「预测占用」：它只对**已落库的分支消息**求投影和（即「下一次请求会带上什么」），不含尚未发送的草稿；两者数据源不同，不得合并实现
- 不实现「继续对话」的摘要注入（I7 续接未纳入当前范围，见任务清单 deferred 段）；§5.4 与 §6 已为该语境预留标注为「暂不生效」的条目，续接上线后无需改契约
- 不改 `TokenUsageDto` 的既有 7 字段结构
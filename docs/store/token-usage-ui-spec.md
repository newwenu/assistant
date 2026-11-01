# Token Usage 语义契约与展示规范

> 术语与计算式的唯一事实源：`docs/store/token-usage-semantics.md`（本文不再重复定义公式）
> 现状诊断：`docs/store/token-usage-analysis.md`
> 本文只定义「字段叫什么、怎么展示、验收判据」，不含实施阶段与任务排期。
> 基线口径：以当前仓库代码为准。

---

## 0 结论速览

| # | 契约 |
| --- | --- |
| 1 | 三个口径必须独立建模：**上下文窗口**（分母）、**上下文占用**（分子）、**累计消耗**（账单）。禁止用累计值填占用位 |
| 2 | **上下文占用**的唯一可信来源是「最近一次聊天请求的 `prompt_tokens`」，不是任何本地求和 |
| 3 | 持久化单元是**请求级记录**（一次 API 调用一条），而不是把 breakdown 塞进 `messages` 行 |
| 4 | `cacheRead` / `cacheWrite` / `reasoning` 必须区分「未上报」与「上报为 0」，用 `NULL` / `undefined` 表达未上报 |
| 5 | 进度条只画**输入口径的占用**（命中 / 写入 / 未缓存三段，和恒等于 `usedTokens`）；输出、推理、累计消耗一律放 Tooltip / 详情层 |

---

## 1 语义契约

### 1.1 三个口径的权威定义

| 口径 | 字段名 | 定义 | 分母/来源 | 用途 |
| --- | --- | --- | --- | --- |
| 上下文窗口 | `contextWindowTokens` | 模型单次请求可容纳的输入+输出上限 | 模型配置 `models_json.contextWindow`，本地注入 | 进度条分母、压缩阈值基准 |
| 上下文占用 | `usedTokens` | 下一次请求将发送的输入 token 数，以「最近一次 `kind = chat` 请求的 `prompt_tokens`」为代理值（两者构成相同） | 最近一次 `kind = chat` 请求的 `prompt_tokens` | 进度条分子、`percentage` |
| 累计消耗 | `cumulativeTokens` | 本会话所有请求 `prompt + completion` 之和 | 请求记录表聚合 | Tooltip / 详情，计费参考 |

`percentage = usedTokens / contextWindowTokens`，clamp 到 `[0, 1]`。

**占用口径选定：输入口径。** `usedTokens` 只含输入，不含本次 `completion`。

理由三条：

1. 进度条回答的是「下一次请求还能装多少」，而 `completion` 已生成完毕、不进入下一次输入；
2. 流式期间本仓库只能拿到 `promptTokens`，若改用含输出口径，同一字段在「流式」与「切会话」后又会分裂成两种语义（即 A1 重演）；
3. 与 openhuman 的 `last_turn_input_tokens`、deepseek-harness-orb 的 `pressureTokens` 同口径，便于横向对照。

含输出口径（codex 的 `last_token_usage.total_tokens`）是另一种合法口径，本文不采用。两种口径相差一个 `completion`，差异与取舍见 `token-usage-semantics.md` §5。输出 token 只通过 Tooltip 展示，不进入进度条。

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

完整禁令表（含文案类）见 `token-usage-semantics.md` §7；下表只列**字段命名**层面的强制项，因为它们直接决定接口契约。禁用以下命名，因为它们在多上下文中含义漂移：

| 禁用 | 原因 | 应改为 |
| --- | --- | --- |
| `totalTokens` 单独出现 | 既可指本请求 total，也可指会话累计 | `requestTotalTokens` / `cumulativeTokens` |
| `usedTokens` 指累计值 | 与「占用」语义冲突 | `cumulativeTokens` |
| 「上下文窗口总 token 花费」 | 窗口与花费是两个量，字面矛盾 | 「窗口占用」或「累计消耗」 |
| `tokens`（裸字段，指整轮 prompt+completion） | 易被误读为「这条消息自己的 token」 | `requestTotalTokens`，并在文档标注其为整轮口径 |

---

## 2 现状基线

本节编号（P / W / Q / F）与 `token-usage-analysis.md` 的 A1–A13 描述的是同一批代码事实的两组切面，映射表见 `token-usage-semantics.md` §8.1。同一事实两处出现时以本节的结构性事实为准，现象描述以 analysis 为准。

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

### 2.3 聚合查询：SUM 混用两种口径

| # | 事实 | 位置 |
| --- | --- | --- |
| Q1 | `TokenUsageSummary` 仅 `total_tokens` / `context_window_tokens` / `percentage` | `assistant-storage/src/agent_modes.rs` |
| Q2 | SQL 为 `SUM(COALESCE(tokens, (LENGTH(content)+3)/4+4))`，把上报值与估算值相加 | 同上 |
| Q3 | 该 SUM 结果被当作「上下文占用」返回 | 同上 |
| Q4 | `TokenUsageSummaryDto` 仅 3 字段，且 `deny_unknown_fields` | `src-tauri/src/protocol/dto/chat.rs` |

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
  kind                  TEXT    NOT NULL,   -- 'chat' | 'compaction'
  provider_id           TEXT,
  model_id              TEXT,
  prompt_tokens         INTEGER NOT NULL,
  completion_tokens     INTEGER NOT NULL,
  cache_read_tokens     INTEGER,           -- NULL = 未上报
  cache_write_tokens    INTEGER,           -- NULL = 未上报
  reasoning_tokens      INTEGER,           -- NULL = 未上报
  context_window_tokens INTEGER,           -- NULL = 该请求未注入窗口，不得用 0 替代
  estimated             INTEGER NOT NULL DEFAULT 0,  -- 1 = 含估算值
  created_at            INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_token_usage_conv
  ON token_usage_records (conversation_id, created_at DESC);
```

字段约束：

| 字段 | 约束 |
| --- | --- |
| `kind` | 决定是否参与占用计算；`compaction` 只参与累计 |
| `prompt_tokens` / `completion_tokens` | 非空；provider 未上报时按 §3.4 回退，并置 `estimated = 1` |
| `cache_read_tokens` 等三项 | 可空，`NULL` 即「未上报」，不得用 0 替代 |
| `attempt_id` | 重试时区分同一轮内的多次请求；每次成功或失败的带 usage 请求都落一条 |
| `message_id` | 关联 assistant 消息；压缩请求指向摘要消息 |
| `context_window_tokens` | 可空。`NULL` = 该请求发送时未注入窗口（如压缩路径），与 `0` 严格区分；`0` 不是合法窗口值，写入时归一化为 `NULL` |
| `estimated` | 任一 token 字段走了估算即为 1；UI 对估算值加 `~` 前缀 |

### 3.3 派生视图（三个口径各一条查询）

```sql
-- 上下文占用：最新一条 chat 请求的 prompt
SELECT prompt_tokens FROM token_usage_records
WHERE conversation_id = ?1 AND kind = 'chat'
ORDER BY created_at DESC LIMIT 1;

-- 累计消耗：所有请求之和（含压缩与重试）
SELECT COALESCE(SUM(prompt_tokens + completion_tokens), 0)
FROM token_usage_records WHERE conversation_id = ?1;

-- 缓存命中率：分母只含「至少上报了一项缓存字段」的请求
SELECT COALESCE(SUM(COALESCE(cache_read_tokens, 0)), 0) AS hit,
       COALESCE(SUM(prompt_tokens), 0)                  AS denom
FROM token_usage_records
WHERE conversation_id = ?1
  AND (cache_read_tokens IS NOT NULL OR cache_write_tokens IS NOT NULL);
```

> 命中率 = `hit / denom`；`denom = 0` 时返回 `None`（不显示），而不是 `0`。
>
> 分母谓词必须是「任一缓存字段非 `NULL`」，不能只判 `cache_read_tokens`：Anthropic 首次请求只报写缓存、不报读缓存，只判读字段会把它错误排除，命中率恒为 0。

### 3.4 回退顺序

`prompt_tokens` 的取值优先级，用于「provider 未上报 usage」的模型：

1. provider 上报的 `prompt_tokens`
2. `total_tokens - completion_tokens`（若 total 有值）
3. 字符估算：ASCII 按 4 字节 ≈ 1 token，非 ASCII 按 3 字节 ≈ 1 token（即 1 字符 ≈ 1 token），再加消息级结构开销

第 3 条是契约目标值；现状 E1 对全部字符统一按 `ceil(len/4) + 4`（字节），不区分字符集。

非 ASCII 系数取中文实测密度的下限（1~1.5 token/字），因此即使按契约目标值仍属**低估**；该偏差是单向的，对 offload 预算会导致漏判、对聚合会导致百分比偏低。取值依据与各参考项目的对照见 `token-usage-semantics.md` §3.5。

估算值必须可辨识：记录时置 `estimated = 1`，UI 对估算值加 `~` 前缀，聚合时可单独统计「含估算的记录占比」。

---

## 4 各层结构规范

### 4.1 Rust 运行时

| 结构 | 契约 |
| --- | --- |
| `StreamOutcome` | 必须完整携带 `prompt_tokens` / `completion_tokens` / `cache_read_tokens` / `cache_write_tokens` / `reasoning_tokens` / `context_window_tokens`，不得只留 total |
| `PartialOutcome` | 同上；流中断时的部分 usage 同样要落记录 |
| `StreamAccumulator` | 每收到一次 `usage` 就整体替换为本轮最新值（usage 是累计快照，不是增量）；`total` 优先取 provider 的 `total_tokens`，缺失时用 `prompt + completion` |
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
| `maxTokens` | `number \| undefined` | 窗口 `W`；窗口未知时不得填缺省值，置 `undefined` | 是 |
| `percentage` | `number \| undefined` | `clamp(usedTokens / maxTokens, 0, 1)`；`maxTokens` 为 `undefined` 时同为 `undefined` | 是 |
| `completionTokens` | `number` | 最近一次请求的输出 `C`，仅 Tooltip 使用 | 是 |
| `cacheReadTokens` | `number` | `CR` | 是，未上报为 `undefined` |
| `cacheWriteTokens` | `number` | `CW` | 是，未上报为 `undefined` |
| `reasoningTokens` | `number` | `RT` | 是，未上报为 `undefined` |
| `cumulativeTokens` | `number` | 累计 `A` | 是 |

### 4.4 窗口解析

`contextWindowTokens` 的解析顺序（前后端必须一致）：

1. 会话所用模型的 `models_json.contextWindow`
2. provider 级默认窗口
3. 以上都取不到 → **未知**，返回 `undefined` / `NULL`，不得回退到硬编码常量

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
| 常驻行（窗口未知） | 只显示 `占用` 绝对 token 数，不画进度条、不显示百分比 | `usedTokens` |
| Tooltip（hover） | 本次输入 / 输出 / 缓存命中 / 缓存写入 / 推理 + 累计消耗 | `usedTokens` / `completionTokens` / `cacheReadTokens` / `cacheWriteTokens` / `reasoningTokens` / `cumulativeTokens` |
| 详情面板（可选展开） | 按请求逐条记录、按 model 聚合 | 记录表查询 |

进度条只表达占用，不表达累计。累计值不得画进同一根进度条。

### 5.2 进度条分段

进度条只画输入，三段宽度之和恒等于 `usedTokens`，与 §1.1 的占用口径严格一致：

| 段 | 宽度 | 颜色 | 显示条件 |
| --- | --- | --- | --- |
| 缓存命中输入 | `cacheReadTokens / maxTokens` | 绿 | `cacheReadTokens > 0` |
| 缓存写入输入 | `cacheWriteTokens / maxTokens` | 青 | `cacheWriteTokens > 0` |
| 未缓存输入 | `(usedTokens - cacheReadTokens - cacheWriteTokens) / maxTokens` | 蓝 | 恒显示 |

必须保留「缓存写入」这一段：`cacheWrite ⊆ usedTokens` 但既不命中也不未缓存，只画命中与未命中两段会让三段之和少一个 `cacheWrite`，条长与 `usedTokens` 对不上。

约束：当 `cacheReadTokens + cacheWriteTokens > usedTokens`（provider 上报矛盾）时，未缓存段按 0 处理，命中段与写入段按比例截断，保证三段之和不超过 `usedTokens`。

输出与推理不进进度条，只放在 Tooltip 单独一行。原因：`completion` 已生成完毕、不进入下一次请求，若画进容量进度条，条长会比 `usedTokens` 多一个 `completion`，与百分比读数对不上。codex 采用含输出口径（`last_token_usage.total_tokens`）属另一种合法选择，本文不采用，理由见 §1.1。

### 5.3 阈值与状态

沿用现有三档，语义不变：

| 区间 | 状态 | 颜色 |
| --- | --- | --- |
| `< 70%` | normal | 主题色 / 分段色 |
| `70% ~ 90%` | warning | 黄 |
| `>= 90%` | danger | 红 |

阈值判定只用 `percentage`（占用口径），与累计值无关。

### 5.4 文案规范

| 场景 | 正确文案 | 禁止文案 |
| --- | --- | --- |
| 进度条 | 「上下文占用 12.3k / 128k（9.6%）」 | 「上下文窗口总 token 花费」 |
| Tooltip 累计行 | 「本会话累计消耗 45.6k」 | 「已用 45.6k」（与占用混淆） |
| 压缩前后 | 「已存档，会话已满」 | 「已压缩，上下文已释放」 |

压缩不改变出站构成（见分析文档 §3.4），文案不得暗示占用会下降。

---

## 6 边界与降级

| 场景 | 行为 |
| --- | --- |
| 会话无任何记录（存量数据） | 唯一允许估算进入 `usedTokens` 的场景：回退到字符估算并标 `~`；或直接显示「—」。有记录时一律用上报值，估算不得覆盖上报值（见 `token-usage-semantics.md` §6 M3） |
| 窗口未知 | `maxTokens` / `percentage` 置 `undefined`，占用显示为绝对 token 数、不画进度条；不得用 128k 冒充分母（见 §4.4）。切换模型 / 会话后若新上下文未声明窗口，必须清空旧窗口而非沿用（M11） |
| provider 不报 usage | 整条链路走估算，`estimated = 1` |
| `cacheReadTokens` 未上报 | 不画命中段，Tooltip 不显示命中行；未缓存段宽度改为 `usedTokens - cacheWriteTokens` |
| `cacheWriteTokens` 未上报 | 不画写入段，未缓存段宽度改为 `usedTokens - cacheReadTokens` |
| `reasoningTokens` 未上报 | Tooltip 不显示推理行 |
| 流式中（usage 尚未到达） | 保持上一次占用值，不归零 |
| 发送瞬间 | 不清空 `usedTokens`，只把状态切到 sending |
| 切换会话 | 先按记录表取占用；取不到时显示「—」，不得先写缺省窗口再异步覆盖 |
| 压缩完成后 | 占用锚点不重置（压缩不缩减出站），仅 `is_full` 置位 |
| 重试 attempt | 每次带 usage 的请求都落记录；占用取最新一条，累计全量相加 |

---

## 7 验收判据

语义层：

1. 同一会话在「流式结束」与「切走再切回」后，`usedTokens` 相同（来自同一条 `prompt_tokens`），不存在两种口径。
2. 128k 窗口、10 轮对话（首轮 `S = 2k`、每轮新增输入 `d = 1k`、每轮输出 `c = 1k`，即每轮净增 2k）的会话：占用 `O = S + 9(d+c) = 20k`（15.6%），累计 `A = 10S + 45(d+c) + 10c = 120k`；两者在 UI 上分处两层，进度条只用前者。推导见 `token-usage-semantics.md` §3.2。
3. `cacheReadTokens` / `cacheWriteTokens` / `reasoningTokens` 未上报时，TS 侧为 `undefined` 而非 `0`。
4. 缓存命中率的分母只含「至少上报一项缓存字段」的请求（§3.3 谓词）。

数据层：

5. 一次成功聊天产生 1 条 `kind = 'chat'` 记录；一次重试产生 2 条（同一 `attempt_id` 前缀不同）；一次压缩产生 1 条 `kind = 'compaction'` 记录。
6. 压缩请求的 token 计入累计，不计入占用。
7. `messages.tokens` 与记录表中的对应请求 total 一致（若保留该列）；无 usage 时该列为 `NULL` 而非估算值。
8. 存量数据库升级后新表为空，旧会话显示「—」，不报错。

展示层：

9. `completionTokens` / `cacheReadTokens` / `reasoningTokens` 全部缺失时，进度条退化为单色，布局不变。
10. 占用下降（如分支切换）后进度条同步下降，不残留旧值。
11. 窗口未知（模型未配置 `contextWindow`）时，`maxTokens` / `percentage` 为 `undefined`，常驻行只显示绝对 token 数，不出现进度条与百分比；切换到未知窗口的模型后，不沿用上一个模型的窗口值。

---

## 8 不做什么

- 不把 breakdown 塞进 `messages` 表（见 §3.1 判据）
- 不实现货币计价与成本估算
- 不实现「预测占用」（按当前编辑中的输入预估下一轮）——deepseek-harness-orb 用 `projectedTokens = pressureTokens + surfaceTokens - sampledSurfaceTokens` 提供该能力，OpenBitFun 用 `TokenAnchor` 的尾部估算等价实现；两者都依赖事件级的 surface 变更记录，本仓库无此基础设施，故不纳入本文契约
- 不改变压缩行为本身（压缩不缩减出站是既有事实，本文只负责不误导用户）
- 不改 `TokenUsageDto` 的既有 7 字段结构
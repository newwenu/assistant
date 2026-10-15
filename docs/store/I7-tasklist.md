# I7 — Context Compaction 任务清单

> 规格：[I7-spec.md](./I7-spec.md)
> 前置：I6 ✅

---

## 阶段 A — 常量 + 表扩展 + CRUD

- [x] A1. 新增常量 `COMPACTION_THRESHOLD_PERCENT = 80` / `MAX_SUMMARY_REQUEST_TOKENS = 4096`（Rust + TS）
- [x] A2. `conversations` 表手动迁移新增 `is_full` 列
- [x] A3. `compression_state` 表结构确认（无 summary 列，摘要存入 messages 表 compact_summary 消息）
- [x] A4. `ConversationRow` 新增 `is_full: bool`，`CONV_SELECT` / `read_conv_row` / insert / update 同步
- [x] A5. 新增 `CompressionStateRow` 结构体（`conversation_id` + `last_compression_at` + `compression_count`，无 summary 字段）
- [x] A6. 新增 `compression_state` CRUD（get / upsert）
- [x] A7. AppConf 新增 `auto_compact: bool` + `compact_threshold_percent: u8` + `compact_summary_prompt: Option<String>` + `compact_summary_max_tokens: u16`（Rust + TS）

---

## 阶段 B — 终止逻辑 + 摘要生成

- [x] B1. 新增默认摘要 prompt 模板常量 `DEFAULT_COMPACT_SUMMARY_PROMPT`
- [x] B2. 新增 `should_terminate` 阈值判断（仅当 usage.prompt_tokens 存在时触发）
- [x] B3. 新增 `terminating` 并发防护标记（`HashSet<String>` 按对话隔离，send_chat_message 入口检查）
- [x] B4. 新增 `generate_summary`：前缀缓存复用 + 流式调用收集完整输出 + 30 秒超时 + 失败回滚
- [x] B5. 新增 `terminate_conversation`：事务写入 is_full=1 + 摘要
- [x] B6. 测试：`should_terminate` 边界值 + usage 缺失不触发

---

## 阶段 C — 流结束触发终止

- [x] C1. `send_chat_message` 新增 `is_full` / `terminating` 守卫
- [x] C2. `stream_future` 流结束后：检查 usage → should_terminate → 执行终止流程
- [x] C3. 新增 `manual_compact_conversation` Tauri command（检查前提 → 执行终止流程）
- [x] C4. 新增 `unlock_conversation` Tauri command（UPDATE is_full=0 + emit 事件）
- [x] C5. 摘要失败时：清除 terminating 标记，日志记录，对话继续，emit `conversation_compact_failed` 事件

---

## 阶段 D — 前端 Store + 事件 + UI

- [x] D1. chatStore 新增 `isFull` + `compactSummary` + `compactError` 状态
- [x] D2. `Conversation` interface 新增 `is_full: boolean`
- [x] D3. 订阅 `conversation_terminated` 事件，更新 chatStore（含 `compactSummary` 赋值）
- [x] D4. 订阅 `conversation_compact_failed` 事件，设置 compactError
- [x] D5. 对话满横幅（[新建对话] + [解锁对话]）— CompactBanner 独立组件；摘要通过 MessageBubble 中 compact_summary 消息渲染，用户可直接查看和复制
- [x] D6. 摘要生成失败横幅（失败原因 + [重试] + [关闭]）— CompactErrorBanner 独立组件
- [x] D7. `chat-tool-bar` [生成交接摘要] 按钮（手动触发，流进行中 disabled）
- [x] D8. 侧边栏已满对话锁图标
- [x] D9. chatService 新增 `manualCompactConversation` / `unlockConversation` invoke 调用

---

## 阶段 E — 设置注册 + i18n

- [x] E1. `defineSetting` 注册 4 个 compaction 设置项（详见 spec §6.2）
- [x] E2. `GeneralSettings.tsx` 追加 `COMPACT_SETTINGS` 到 SettingsGroup
- [x] E3. i18n 中英文键（settings.general.compact.* + chat.compact.* + chat.input.placeholder_full，详见 spec §6.3）

---

## 阶段 F — 集成验证

- [ ] F1. lint + typecheck + clippy + test 通过
- [ ] F2. 验收标准 spec §10 全部通过
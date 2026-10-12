# I7 — Context Compaction 任务清单

> 规格：[I7-spec.md](./I7-spec.md)
> 前置：I6 ✅

---

## 阶段 A — 常量 + 表扩展 + CRUD

- [ ] A1. 新增常量 `COMPACTION_THRESHOLD_PERCENT` / `MAX_SUMMARY_REQUEST_TOKENS`
- [ ] A2. `conversations` 表新增 `is_full` 列
- [ ] A3. `compression_state` 表新增 `summary` 列
- [ ] A4. `ConversationRow` / `CompressionStateRow` 同步扩展
- [ ] A5. 新增 `search_history_messages` 检索函数（tool-call 注册待后续）
- [ ] A6. 新增 `compression_state` CRUD
- [ ] A7. AppConf 新增 `auto_compact` + `compact_threshold_percent`（Rust + TS）

---

## 阶段 B — 终止逻辑 + 摘要生成

- [ ] B1. 新增 `SUMMARY_SYSTEM_PROMPT` 常量
- [ ] B2. 新增 `should_terminate` 阈值判断
- [ ] B3. 新增 `generate_summary` 非流式调用
- [ ] B4. 测试：`should_terminate` 边界值

---

## 阶段 C — 流结束触发终止 + 检索 command

- [ ] C1. `send_chat_message` 新增 `is_full` 守卫
- [ ] C2. `stream_future` 流结束后检查 + 触发终止
- [ ] C3. 新增 `search_history` Tauri command（tool-call 注册待后续）
- [ ] C4. 新增 `get_compression_state` Tauri command

---

## 阶段 D — 前端 Store + 事件 + UI

- [ ] D1. chatStore 新增 `isFull` 状态
- [ ] D2. `conversation_terminated` 事件处理
- [ ] D3. 对话满横幅（摘要 + 复制按钮 + 新建对话按钮）
- [ ] D4. 侧边栏已满对话 `<LockOutlined />` 图标

---

## 阶段 E — 设置注册

- [ ] E1. `defineSetting` 注册 `auto_compact` + `compact_threshold_percent`

---

## 阶段 F — 集成验证

- [ ] F1. lint + typecheck + clippy + test 通过
- [ ] F2. 验收标准 §5 全部通过
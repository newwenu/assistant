# 超长文件拆分规划

> 生成日期: 2026-10-24
> 范围: `src-tauri/` 下所有 Rust 源文件（排除 tests/examples/target）
> 阈值: 仅收录 ≥ 500 行的文件

---

## 1. 现状总览

| Crate | 文件数 | 总行数 | 最长文件 | 平均行数 |
|---|---|---|---|---|
| assistant-providers | 41 | 7,997 | 686 | 195 |
| src-tauri/src (cmd) | 38 | 5,529 | 475 | 145 |
| assistant-security | 31 | 4,778 | 404 | 154 |
| assistant-storage | 11 | 1,952 | **1,179** | 177 |
| assistant-sync | 8 | 1,444 | 357 | 180 |
| assistant-tools | 8 | 697 | 448 | 87 |
| assistant-agent | 6 | 660 | 338 | 110 |
| assistant-config | 7 | 539 | 151 | 77 |
| assistant-orb | 4 | 362 | 183 | 90 |
| assistant-i18n | 2 | 97 | 83 | 48 |

**拆分阈值参考**（Rust 社区惯例）:
- ✅ < 500 行: 暂不拆分
- 🟡 500–600 行: 建议拆分
- 🔴 > 600 行: 强烈建议拆分
- 🚨 > 1000 行: 必须拆分

---

## 2. 待拆分文件清单

按优先级从高到低排列。

---

### P0 — 必须拆分

#### 2.1 `assistant-storage/src/conversations.rs` — 1,179 行 🚨

**当前结构**（48 个公开/私有项）:

| 行号区间 | 内容 | 估算行数 |
|---|---|---|
| 1–84 | 类型定义 `ConversationRow`, `MessageRow`, `ConversationDetail` + 行读取辅助 `read_conv_row`, `read_msg_row` | ~84 |
| 85–187 | **会话 CRUD**: `insert_conversation`, `get_conversations`, `get_conversation_by_id`, `get_conversation_detail`, `update_conversation`, `delete_conversation` | ~102 |
| 188–278 | **消息 CRUD**: `insert_message`, `get_messages`, `update_message_content`, `get_messages_paginated`, `get_message_count` | ~90 |
| 278–458 | **树/子树操作**: `get_subtree_size`, `compact_sibling_indices`, `delete_subtree`, `find_new_active_leaf` | ~180 |
| 458–627 | **会话辅助操作**: `search_conversations`, `export_*`, `toggle_pin`, `duplicate_conversation`, `rename_conversation`, `batch_delete`, `purge_all`, `increment_generation` | ~170 |
| 627–733 | **压缩状态**: `CompressionStateRow`, `get_compression_state`, `upsert_compression_state`, `set_conversation_is_full`, `is_conversation_full` | ~106 |
| 733–852 | **分支追踪**: `trace_active_branch`, `SiblingInfo`, `get_message_siblings` | ~119 |
| 852–1013 | **分支创建/切换**: `MaxSiblingsExceededError`, `create_branch`, `update_active_branch_id`, `switch_branch`, `find_branch_leaf` | ~161 |
| 1013–1179 | **消息查询 + 分叉**: `get_conversation_id_by_message`, `get_message_by_id`, `get_sibling_count`, `get_sibling_counts_batch`, `fork_to_new_conversation` | ~166 |

**拆分方案**: 将 `conversations.rs` 改为 `conversations/mod.rs`，拆为以下子模块:

```
conversations/
├── mod.rs          — re-exports + 类型定义 (ConversationRow, MessageRow, ConversationDetail)
│                     + CONV_SELECT/MSG_SELECT 常量 + read_conv_row/read_msg_row 辅助
│                     + get_conversation_detail 组合入口 (~120 行)
├── crud.rs         — insert/get/update/delete conversation (~110 行)
├── msg_ops.rs      — 所有消息操作: insert_message, get_messages,
│                     update_message_content, get_messages_paginated,
│                     get_message_count, get_message_by_id,
│                     get_conversation_id_by_message,
│                     get_sibling_count, get_sibling_counts_batch (~200 行)
├── tree_ops.rs     — get_subtree_size, compact_sibling_indices,
│                     delete_subtree, find_new_active_leaf (~180 行)
├── ops.rs          — search_conversations, export_conversation_markdown,
│                     export_conversation_json, toggle_pin,
│                     duplicate_conversation, rename_conversation,
│                     batch_delete, purge_all, increment_generation,
│                     fork_to_new_conversation (~200 行)
├── compression.rs  — CompressionStateRow, read_compression_state_row,
│                     get/upsert_compression_state,
│                     set/is_conversation_full (~106 行)
└── branch.rs       — trace_active_branch, find_branch_leaf,
                      SiblingInfo, get_message_siblings,
                      MaxSiblingsExceededError, create_branch,
                      update_active_branch_id, switch_branch,
                      MAX_BRANCH_DEPTH, MAX_SIBLINGS (~220 行)
```

**跨模块依赖图**（无环）:

```
mod.rs ──────→ crud.rs, msg_ops.rs, branch.rs   (get_conversation_detail 组合调用)
tree_ops.rs ─→ msg_ops.rs, branch.rs            (delete_subtree→get_message_by_id, find_new_active_leaf→find_branch_leaf)
ops.rs ──────→ crud.rs, msg_ops.rs, branch.rs   (duplicate→insert_conversation+insert_message; fork→trace_active_branch+insert_conversation+insert_message)
branch.rs ───→ crud.rs, msg_ops.rs              (create_branch→get_conversation_by_id+get_message_by_id)
crud.rs ─────→ (无子模块依赖)
msg_ops.rs ──→ (无子模块依赖)
compression.rs→ (无子模块依赖)
```

**拆分难度**: ⭐⭐ 中等
- 所有函数签名已清晰，参数均为 `&Connection` + 数据，无跨模块内部状态依赖
- 需处理: `CONV_SELECT` / `MSG_SELECT` 常量 + `read_conv_row` / `read_msg_row` 辅助函数的共享（放入 `mod.rs`）
- 需处理: `get_conversation_detail` 是组合函数（先查会话→查消息→追踪分支），放入 `mod.rs` 避免 crud ↔ branch 循环依赖
- 需处理: `fork_to_new_conversation` 与 `duplicate_conversation` 同属"派生新会话"，归入 `ops.rs`
- 需处理: `MAX_BRANCH_DEPTH` 随 `trace_active_branch`/`find_branch_leaf` 放 `branch.rs`；`MAX_SIBLINGS` 随 `create_branch` 放 `branch.rs`
- 需更新: `assistant-storage/src/lib.rs` 无需改动（`pub mod conversations;` 对目录模块自动生效）
- 需更新: 外部 crate 对 `conversations::X` 的引用路径不变

**风险点**:
- `create_branch` 内部调用 `get_message_by_id` + `get_conversation_by_id`，拆分后需跨模块调用（通过 `super::`）
- `delete_subtree` 调用 `compact_sibling_indices`，两者同属 `tree_ops` 则无问题
- `get_conversation_detail` 调用 `trace_active_branch` + `get_messages`，放入 `mod.rs` 作为顶层组合入口，避免 crud ↔ branch 循环

---

#### 2.2 `assistant-providers/src/openai/responses_stream.rs` — 686 行 🔴

**当前结构**:

| 行号区间 | 内容 | 估算行数 |
|---|---|---|
| 1–26 | imports | ~26 |
| 27–55 | `InProgressToolCall` struct + impl | ~28 |
| 56–116 | 辅助函数: `responses_completed_tool_call_completion`, `completed_replay_capture`, `extract_api_error_message` | ~60 |
| 117–140 | `separate_reasoning_summary_part` 辅助函数 | ~23 |
| 140–686 | **`responses_stream()` 主函数** — 包含 retry 循环、SSE 解析、事件分发、tool call 累积、delta 生成 | ~546 |

**拆分方案**:

```
openai/
├── responses_stream/
│   ├── mod.rs           — pub re-exports + `responses_stream()` 入口函数 (~100 行)
│   ├── in_progress.rs   — InProgressToolCall struct + impl (~40 行)
│   ├── helpers.rs       — completed_tool_call_completion, completed_replay_capture, extract_api_error_message, separate_reasoning_summary_part (~90 行)
│   └── event_handler.rs — SSE 事件处理逻辑（从 `responses_stream` 的 async_stream 宏观体中提取事件分发分支）(~350 行)
```

**拆分难度**: ⭐⭐⭐ 较高
- **核心难点**: 主函数使用 `async_stream::stream!` 宏，整个流生命周期在一个大闭包内，状态变量（`tool_calls_by_output_index`, `done_items`, `unified_responses` 等）跨事件共享
- `async_stream::stream!` 内部无法轻易拆分为独立函数调用，因为 yield 点必须在宏内
- **可行策略**: 将事件分发逻辑提取为返回 `Option<UnifiedResponse>` 或 `Vec<UnifiedResponse>` 的纯函数，主循环只做 yield。将 `response.completed` / `response.failed` / `response.incomplete` / `response.output_item.done` 等分支各抽为函数
- 需处理: `InProgressToolCall` 的可变状态与 `tool_calls_by_output_index` HashMap 的交互

**风险点**:
- 流宏内的 `yield` 语法无法移出，拆分后主循环仍需保留骨架
- 过度拆分可能降低流逻辑的可读性，需权衡

---

## 3. 执行计划

### Phase 1 — 核心拆分（预计 1 天）

| 序号 | 文件 | 拆分后最大单文件 | 难度 |
|---|---|---|---|
| 1 | `conversations.rs` → 目录模块 | 220 行 (branch.rs) | ⭐⭐ |

**验证**: `cargo check` + `cargo test` + 确认所有 `conversations::X` 外部引用路径不变。

### Phase 2 — 流逻辑拆分（预计 1–2 天，需验证方案）

| 序号 | 文件 | 策略 | 难度 |
|---|---|---|---|
| 2 | `openai/responses_stream.rs` | 提取事件处理分支为纯函数 | ⭐⭐⭐ |

**验证**: 先做小规模提取（如 `response.completed` 分支），确认 `async_stream::stream!` 内调用外部函数可行后再推广。

### Phase 3 — 观察期

对其余 400–499 行文件持续监控，若超过 500 行则纳入下一轮拆分：
- `cmd/conversation.rs` (475)
- `os/fs_link.rs` (448)
- `gemini/request.rs` (447)
- `gemini/message_converter.rs` (407)
- `screen_lock/mod.rs` (404)
- `anthropic/request.rs` (399)
- `tool_call_accumulator.rs` (398)
- `gemini/stream.rs` (389)
- `client/quirks.rs` (377)
- `security_db/crud.rs` (368)
- `vault_file/mod.rs` (357)
- `cmd/compaction.rs` (356)

---

## 4. 拆分通用规范

### 4.1 目录模块模式

将 `foo.rs` 改为 `foo/mod.rs` + 子模块时:

1. 原 `foo.rs` 头部 imports 移入 `mod.rs`
2. 类型定义 + 常量 + 辅助函数保留在 `mod.rs`，用 `pub(crate)` 暴露
3. 各子模块通过 `use super::` 引用共享项
4. `mod.rs` 用 `pub use submodule::*;` re-export 保持外部 API 不变

### 4.2 不变量检查

每个拆分完成后必须通过:
- `cargo check` — 编译通过
- `cargo test` — 全部测试通过
- `cargo clippy` — 无新 warning
- 外部 crate 对拆分模块的引用路径不变（通过 re-export 保证）

### 4.3 禁止事项

- ❌ 不为拆分而拆分：500 行以下的文件不拆
- ❌ 不改变公开 API：外部可见的函数签名、类型定义不变
- ❌ 不引入循环依赖：子模块间通过 `super::` 单向引用
- ❌ 不在 `async_stream::stream!` 宏外使用 yield

---

## 5. 预期收益

| 指标 | 拆分前 | 拆分后（Phase 1-2） |
|---|---|---|
| 超 500 行文件数 | 2 | 0 |
| 超 1000 行文件数 | 1 | 0 |
| 最大单文件行数 | 1,179 | ~220 |
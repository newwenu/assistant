# 超长文件拆分与职责归位规划

> 更新日期: 2026-11-07
> 性质: **执行计划**（待办方案 + 已完成基线 + 进度）。拆分判据、目录模块模式、禁止事项的唯一来源是 [`docs/principle/CODE-SPLIT-PRINCIPLE.md`](../principle/CODE-SPLIT-PRINCIPLE.md)，本文不复述条款。
> 范围: `src-tauri/` 下全部 `.rs`（排除 `tests/`、`examples/`、`target/`）
> 统计口径: 非空行（含注释与内联测试），2026-11-07 快照，仅用于初筛定位与横向对比，允许偏差，不作为验收标准。

---

## 1. 待办

排序 = 职责数 × 跨模块耦合度 × 修改频度；行数只作辅助权重。

### 1.1 D 阶段：需引入状态结构的真重构

两者都要新增状态结构，属真重构，需独立 PR + 回归验证，排在 E 阶段之后。

#### `assistant-providers/openai/responses_stream.rs`（688）

现状：`responses_stream()` 主函数承担了全部职责，内含 retry 循环、SSE 解析、`response.completed` / `failed` / `incomplete` / `output_item.done` 分支、tool call 累积（`InProgressToolCall` + `tool_calls_by_output_index`）。文件内另有 `InProgressToolCall::from_item_value`、`responses_completed_tool_call_completion`、`completed_replay_capture`、`extract_api_error_message`、`separate_reasoning_summary_part`。

拆分方案：

```
openai/responses_stream/
├── mod.rs           — pub(crate) re-export + responses_stream() 入口（retry 循环 + 组装）
├── in_progress.rs   — InProgressToolCall + from_item_value
├── helpers.rs       — responses_completed_tool_call_completion / completed_replay_capture
│                      / extract_api_error_message / separate_reasoning_summary_part
└── event_handler.rs — handle_event(&mut StreamState, &Value) -> Vec<UnifiedResponse>
                       （事件分发、tool call 累积、delta 生成）
```

要点：

- `async_stream::stream!` 内的 `yield` 不能移出宏，因此策略是**把状态与分支逻辑抽成返回 `Vec<UnifiedResponse>` 的纯函数**，宏内只保留 `for event in ... { for r in handle_event(...) { yield r } }`。
- `StreamState` 需新定义为独立结构（承载 `tool_calls_by_output_index`、`done_items`、`unified_responses`），这是本次拆分的核心重构点。
- 先做小步验证：只抽 `response.completed` 分支，确认 `cargo check` 通过后再推广。
- 事件分发顺序、delta 生成时机的语义极易漂移。建议先补 `handle_event` 级 fixture 再动手，拆分后可复用。

难度：高。

#### `assistant-agent/multimodal.rs`（761）

现状（按职责）：

| 职责段 | 内容 |
|---|---|
| 配置与信任级别 | `MultimodalConfig` / `trusted_channel_input` / `untrusted_channel_input` |
| 校验与 MIME | `validate_path` / `media_type_from_ext` / `file_media_type_from_ext` / `ext_of` / `file_name_from_path` |
| 消息内容块读写 | `apply_blocks` / `blocks_of` |
| 准入 | `admit_attachments` / `admit_image` / `admit_file` |
| 重水化 | `rehydrate_images` / `rehydrate_files` |
| 投影 | `project_images_for_text_model` / `project_files` / `project_file_block` |
| 文本回退 | `is_text_file` / `read_as_text` / `file_fallback_text` / `should_send_file_as_is` / `text_only_image_text` / `offloaded_image_text` / `file_handle_text` |

拆分方案：

```
assistant-agent/src/multimodal/
├── mod.rs       — MultimodalConfig + trusted/untrusted + re-export
├── validate.rs  — validate_path / media_type_from_ext / file_media_type_from_ext
│                  / ext_of / file_name_from_path
├── blocks.rs    — apply_blocks / blocks_of
├── admit.rs     — admit_attachments / admit_image / admit_file
├── rehydrate.rs — rehydrate_images / rehydrate_files
├── project.rs   — project_images_for_text_model / project_files / project_file_block
└── text.rs      — is_text_file / read_as_text / file_fallback_text / should_send_file_as_is
                   / text_only_image_text / offloaded_image_text / file_handle_text
```

要点：`admit.rs` 是最大子模块，若后续继续增长，再按 image / file 一分为二；`project.rs` 依赖 `text.rs`，单向。该模块现无自动化测试，拆分后需按"图片/文件/纯文本模型/PDF"四类场景手测。

难度：中高（无自动化测试兜底，语义漂移只能靠手测发现）。

### 1.2 E 阶段：P2 归位清单

次序：**仅移测试 → 平台拆分 → 领域拆分**，低风险项先行，控制单 PR 体积。

| 文件（非空行） | 职责混杂点 | 目标结构 |
|---|---|---|
| `assistant-sync/git_sync/git.rs`（554） | 44 个 git 命令封装：仓库、分支、远端、日志、底层 `Command` 拼装 | `git/{mod,repo,branch,remote,log,plumbing}.rs` |
| `src-tauri/src/cmd/conversation.rs`（527） | 20 个 tauri command + DTO 组装（`build_detail_dto`）+ 三种分页变体 + 分支操作 | `cmd/conversation/{mod,list,detail,detail_dto,mutate,branch,export}.rs`；DTO 组装移入 `detail_dto.rs` |
| `assistant-providers/gemini/message_converter.rs`（525） | 消息 / 工具 / schema 三段转换 | `message_converter/{messages,tools,schema}.rs` |
| `src-tauri/src/protocol/dto/chat.rs`（598） | 会话 / 消息 / token 用量 / 事件载荷 / 附件引用 / 内容块 DTO | `dto/chat/{conversation,message,usage,event,content}.rs` |
| `assistant-vault/dto/convert.rs`（434） | 5 类共 23 个 `From` / `TryFrom`（sync 15、totp 3、error 2、hibp 1、entry 2） | `dto/convert/{sync,entry,totp,error,hibp}.rs`；`sync` 再按 `conflict / history / config / pending` 细分（该文件无 `vault_meta` 领域） |
| `assistant-providers/anthropic/request.rs`（405） | 与 gemini 同构：请求体组装 + 发送 + 流包装 | `anthropic/request/{mod,body,stream}.rs` |
| `assistant-vault/vault_file/mod.rs`（393） | 条目文件读写 + vault_meta 签名/校验 + 应急数据 + 主密钥重写 | `vault_file/{mod,entry,meta,emergency}.rs` |
| `src-tauri/src/cmd/vault/sync.rs`（387） | 20 个 command 覆盖同步、Git 远端、历史、冲突四类 | `cmd/vault/sync/{mod,git,history,conflict}.rs` |
| `assistant-providers/gemini/request.rs`（373） | `chat_completion_stream` 单函数（请求体组装 + 发送 + 流包装）+ URL 推导 | `gemini/request/{mod,body,stream,url}.rs` |
| `src-tauri/src/cmd/compaction.rs`（353） | 2 个入口 command + `execute_compaction` 单函数（提示词构建、provider 调用、流消费、落库）+ 部分结果落库 | `cmd/compaction/{mod,pipeline,persist}.rs`；`execute_compaction` 按"构建提示 → 调 provider → 消费流 → 落库"四段拆 |
| `assistant-tools/os/agent_grep.rs`（747） | 工具契约、输出渲染（行上限/超时 footer）、请求解析（include 合并、`base_directory`）、遍历与文件过滤策略、`grep-searcher` 接线（`FileSink` + 6 个 `Sink` 回调）、非 UTF-8 前缀探测（冷路径） | `os/agent_grep/{mod,request,outcome,search,sink,probe,render}.rs`；`outcome.rs` 为共享类型叶子，`SearchOutcome` 字段需由私有改 `pub(crate)` |
| `assistant-tools/os/agent_delete.rs`（571） | 安全边界（`resolve_target` / `reject_undeletable` / `same_path` / `home_dir`）、平台属性（只读与链接判定）、预扫描与审批预演、删除执行（`delete_path` / `remove_link` / `ensure_empty_directory` / `describe_failure`）、工具契约 | `os/agent_delete/{mod,target,attrs,scan,exec}.rs`；`attrs.rs` 为 `scan` 与 `exec` 共用叶子，2 项内联测试测私有函数，保留在源文件内 |
| `assistant-tools/gateway.rs`（561） | 审批键归一化（`ApprovalScope` / `ApprovalKey` + `canonical_path_for_key` / `split_argv` / `command_cwd` / `parse_endpoint` / `opaque_fingerprint`）、网关决策与审计、三个内存存储实现、测试替身 | `gateway/{mod,approval_key,stores,testing}.rs`；`ApprovalScope` / `ApprovalKey` 必须与归一化规则同处 `approval_key.rs`（分开会让类型留在编排层、规则留在叶子层而互相 `use super`）；`ScriptedApprovalPrompter` 被 `tests/gateway.rs`、`tests/pipeline.rs` 当公开 API 使用，不能加 `#[cfg(test)]`，只能 `pub use` |
| `assistant-tools/os/agent_write.rs`（531） | 编码策略（`TargetEncoding` + `resolve_target_encoding`）、原子写机制（`write_atomically` / `build_payload` / `read_existing` / `too_large_message`）、行数统计纯函数（`count_lines` / `count_lines_in_file`）、审批预演 | `os/agent_write/{mod,encoding,atomic,lines,preview}.rs`；`lines.rs` 必须独立成叶子，否则 `atomic` 与 `preview` 会为 `count_lines` 互相依赖 |

上述 4 项（`assistant-tools` 工具层，按建议执行次序排列）的共同前提：先把 3 处已复制的实现下沉为跨工具共享叶子，再动手拆分，否则拆完只是把第二份拷贝搬进子模块。先例是 `os/agent_glob.rs` 的 `build_glob_set` 已是 `pub(crate)` 并被 `agent_grep` 复用，属本 crate 既有做法，不触碰对外公开 API。

| 重复项 | 现状 | 建议落点 |
|---|---|---|
| `base_directory` | `agent_grep.rs` 与 `agent_glob.rs` 逐字相同 | `os/scope.rs`（`pub(crate)`） |
| `ignore::WalkBuilder` 配置 | `agent_grep.rs` 与 `agent_glob.rs` 是同一组 `.hidden` / `.git_ignore` / `.follow_links` 开关 | `os/walk.rs`（`pub(crate)`） |
| `ensure_empty_directory` | `agent_delete.rs` 与 `agent_move.rs` 逻辑同构，仅错误文案不同 | `os/fs_util.rs`（带 `noun` 参数） |

拆分次序：`agent_grep`（零内联测试、以纯搬移为主）→ `agent_delete`（安全边界独立，收益最高）→ `agent_write` → `gateway`（公开 API 需原样 re-export）。

注意：`agent_write` 与 `agent_edit/report.rs` 的审批预演文案用中文，`agent_delete` 的 `describe_preview` 用英文。若审批弹窗直接展示 `summary`，应先统一目标语言再拆，避免搬移时顺手"修正"其中一侧，把语义漂移混进结构重构的 diff。

已从本清单移出的项：

| 项 | 结论 |
|---|---|
| `client/quirks.rs`、`protocol/dto/mod.rs` | 仅移测试，已完成，见 §2 |
| `screen_lock/mod.rs`、`os/fs_link.rs` | 平台拆分，已完成，见 §2 |
| `tool_call_accumulator.rs`、`assistant-agent/chat_runtime.rs` | 文件已不存在（前者累积器迁至 `assistant-agent/src/agent/tool_call.rs`，后者 P3 重构后删除），本项作废 |

### 1.3 P3 观察名单（职责单一，暂不动）

`gemini/stream.rs`、`cmd/vault/entries.rs`、`openai/chat.rs`、`gemini/code_assist.rs`、`openai/stream.rs`、`stream/inline_think.rs`、`types/chat_delta.rs`、`openai/responses_types.rs`、`src/window.rs`、`security_db/mod.rs`、`assistant-storage/agent_modes.rs`。

触发条件：任一文件出现第二个变化原因（职责数增至 ≥3）时纳入下一轮；**不因行数增长自动触发**。

`assistant-tools` 工具层本轮新增观察项：

| 文件（非空行 / 其中内联测试） | 现状 | 升级条件 |
|---|---|---|
| `os/path.rs`（647 / ~295） | 仅两条变化原因：路径归一化（`resolve` / `canonicalize_lenient`）与安全名单（受保护目录、凭据目录、`ensure_within_root`） | 安全名单继续扩张时，下沉 `os/path/protected.rs` |
| `os/text_encoding.rs`（430 / ~100） | `decode` 与 `encode` 可分，但共享 `Encoding` 枚举与 BOM 语义，拆分收益有限 | 新增编码探测规则时再动 |
| `os/agent_read.rs`（325） | 唯一可独立的是回执 guard（读前置校验 + 编码继承），但仅几十行且与读取结果强耦合 | 职责数增至 ≥3 时 |

已核但确认不拆：`pipeline.rs`（350，2 条变化原因）、`registry.rs`（317，其中约 196 行为内联测试，实现单职责）、`os/agent_list.rs` / `os/agent_move.rs` / `os/agent_glob.rs` / `os/agent_mkdir.rs` / `os/output.rs` / `os/process_output.rs`（均 <300 行）。

---

## 2. 已完成基线（不得回退）

| 项 | 最终结构 | 基准（最长子模块 / 原文件） |
|---|---|---|
| Phase 1：`storage/conversations.rs` | 目录模块：mod / crud / msg_ops / tree_ops / ops / compression / branch | storage 最长文件为 `agent_modes.rs` |
| I21：`send_chat_message` 上帝函数 | `cmd/chat/{mod,insert,stream}`，主函数收窄为编排 | 原单函数拆为 Pipeline |
| I22：`chat/request.rs` + `chat/utils.rs` 下放 `assistant-agent` | `chat_request.rs` + `chat_runtime.rs`；`build_provider_config` 收敛 3 调用点 | `cmd/conversation_utils.rs` 仅剩薄封装 |
| Phase A：`security_db/crud.rs`（1198） | `security_db/entries/{mod,tx,field_crypto,snapshot,query,write,deleted,import}.rs` | `write.rs` 288 |
| Phase B：`sync_engine/mod.rs`（1149） | `sync_engine/{mod,preview,lifecycle,export,pull_push,remote_admin,history,emergency}.rs` | `mod.rs` 33 / `history.rs` 327 |
| Phase C：`unlock_dialog/windows/mod.rs`（954） | `windows/{mod,ffi,state,text,subclass,paint,wm_handlers,wnd_proc}.rs` + 既有 layout/theme/localization | `mod.rs` 162 / `wm_handlers.rs` 352 |
| E-1：`client/quirks.rs` 测试外移 | `assistant-providers/tests/quirks.rs`（9 项） | 实现 377 → 204 |
| E-1：`protocol/dto/mod.rs` round-trip 外移 | `src-tauri/tests/protocol_dto_roundtrip.rs` | 实现 20 行 re-export |
| E-2：`screen_lock/mod.rs` 平台拆分 | `screen_lock/{mod,windows,macos,linux}.rs` | `mod.rs` 56 / `windows.rs` 267 |
| E-2：`os/fs_link.rs` 职责拆分 | `os/fs_link/{mod,validate,create,check}.rs` | `mod.rs` 175 / `check.rs` 143 |

**回退防线**（这几条是拆分时踩过的坑，合回单文件时会立刻复发）：

- Phase A：`tx.rs` 只放 `with_transaction`，四个 `*_tx` 与对应 `*_in` 同处 `write.rs`（否则 `tx ↔ write` 成环）；`query_ids` 下沉 `entries/mod.rs`（放 `query.rs` 会让 `query ↔ snapshot` 成环）。
- Phase B：`readyness` / `prepare` / `finalize` 独立为 `lifecycle.rs` 叶子（挂任一编排子模块都会让另一侧产生"历史依赖编排"的反语义边）；pending removals 三件套在 `pull_push.rs`；`export_all_entries` 在 `export.rs`（放 emergency 会产生"编排依赖应急"的反向边）。
- Phase C：6 个 `static mut` 必须集中 `state.rs`（按函数就近定义会产生 `wnd_proc ↔ subclass` 双向 `use super::`）；`wnd_proc` 只留 `match msg` + `unsafe` 边界，业务委托 `wm_handlers.rs`。
- E-2：`fs_link` 的 `check` 必须用 `resolve_leaf_preserving`（`canonicalize` 会穿过链接解析到真实目标，对判断"叶子是不是链接"是错的）。

上述文件在后续职责数增长时，按本方案重新入观察。

---

## 3. 执行计划与进度

| Phase | 目标 | 顺序理由 | 难度 |
|---|---|---|---|
| A ✅ | `security_db/crud.rs` → `entries/` | 纯机械搬移，无并发与流状态；先建立"拆一个文件一个 PR"的节奏 | 低 |
| B ✅ | `sync_engine/mod.rs` → 8 文件 | 与 A 同 crate，可复用 A 验证过的子模块范式；主要风险是 `SyncEngine` 私有字段跨模块访问 | 中 |
| C ✅ | `unlock_dialog/windows/mod.rs` → 8 文件 | 全文件为 `unsafe` 原生窗口代码，需预留手测时间；先纯搬移、再拆 `wnd_proc` | 高 |
| D | `responses_stream.rs` + `multimodal.rs` | 两者都需引入状态结构，属真实重构，需独立 PR + 回归验证 | 高 |
| E 🔄 | P2 清单按"仅移测试 → 平台拆分 → 领域拆分"次序推进 | 低风险项先行，控制单 PR 体积 | 中 |

E 阶段进度（2026-11-07）：

| 次序 | 项 | 状态 |
|---|---|---|
| 1 仅移测试 | `client/quirks.rs` 测试外移 `tests/quirks.rs` | ✅ 9 项通过，实现文件 377 → 204 |
| 1 仅移测试 | `protocol/dto/mod.rs` round-trip 外移 | ✅ 已在 `src-tauri/tests/protocol_dto_roundtrip.rs` |
| 2 平台拆分 | `screen_lock/mod.rs` → `{mod,windows,macos,linux}.rs` | ✅ `cargo check -p assistant-security --all-targets` 通过 |
| 2 平台拆分 | `os/fs_link.rs` → `{mod,validate,create,check}.rs` | ✅ 14 项测试通过（含测试外移至 `tests/fs_link.rs`） |
| 3 领域拆分 | 其余条目（见 1.2） | 待做 |

执行节奏：**先纯搬移，再拆函数骨架**（如 `wnd_proc` 只留 `match msg` + `unsafe` 边界，业务委托给普通函数）；需引入新状态结构的拆分（如 `responses_stream` 的 `StreamState`）属真重构，应独立提交并先补 fixture 再动手。

每个 Phase 的完成判据（在 `src-tauri` 目录执行）：

```
cargo check
cargo test
cargo clippy --all-targets -- -D warnings
```

注意：`assistant-vault` 的 `tests/sync_engine.rs` 挂在 `#![cfg(feature = "git-tests")]`；`tests/vault_crud.rs` 与 `tests/vault_sync_db.rs` 整体挂在 `#![cfg(feature = "slow-tests")]`，`assistant-crypto` 的 `tests/vault_crypto.rs` 中 8 个走真实 Argon2 的用例按用例挂 `#[cfg(feature = "slow-tests")]`。门禁需显式开启，否则对应测试会显示 `running 0 tests` 或被过滤：

```
cargo test -p assistant-vault --features slow-tests
cargo test -p assistant-vault --features git-tests --test sync_engine
cargo test -p assistant-crypto --features slow-tests --test vault_crypto
```

Phase C 与 `multimodal` 无自动化测试覆盖，除上述三条命令外还需手工回归：解锁流程（输入 / 粘贴 / 回车 / Esc / 可见性切换 / DPI 缩放 / 深色主题 / 按钮 hover 与焦点框）；附件四类场景（图片 / 文件 / 纯文本模型 / PDF）。

---

## 4. 拆分规范

拆分判据、目录模块模式、职责边界判据、依赖方向与禁止事项的唯一来源是 [`docs/principle/CODE-SPLIT-PRINCIPLE.md`](../principle/CODE-SPLIT-PRINCIPLE.md)；本文不复述条款，只保留与具体文件相关的归属结论。
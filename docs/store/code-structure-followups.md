# 代码结构优化后续待办

| 项 | 值 |
|---|---|
| 状态 | 待办（活跃） |
| 日期 | 2026-11-16 立；2026-11-17 复核并重排 |
| 来源 | 代码组织与深度扁平度优化规格（已归档）的未完成项 |
| 判据来源 | 文件拆分判据唯一来源为 [CODE-SPLIT-PRINCIPLE.md](../principle/CODE-SPLIT-PRINCIPLE.md)；结构事实唯一来源为 `scripts/probe-structure.mjs` |
| 关联 | [CODE-STRUCTURE.md](../CODE-STRUCTURE.md)（现状事实）、[crate-architecture.md](./crate-architecture.md)（crate 边界） |

> 本文只承载「尚未完成的事项」。规格本体已归档冻结，其 §2.7 / §2.8 两张清单是 2026-11-16 的历史快照，**不得当作现状引用**（失效原因见 F3）。
> 结构数字一律以 `node scripts/probe-structure.mjs` 的输出为准；本文只记录复核当时的实测值并标注时刻，不维护第二套统计口径。

---

## 0 复核结论（2026-11-17）

复核方式：跑一次 `probe-structure.mjs`，对全仓 `src/**/*.rs` 与非测试前端文件逐文件清点非空行，并打开 F1 点名的两个文件及其测试核对。四条原条目的复核结果：

| 原条目 | 复核结果 |
|---|---|
| F1 | 两个文件行数无误（692 / 688）；但 `multimodal.rs`「无自动化测试覆盖」不成立，前置条件需缩小 |
| F2 | 已收口：口径与非空行数字随本次结构文档批提交（见 F2 节），后续重刷降为随批动作 |
| F3 | 账目错误。归档规格 §2.7 有 21 条、§2.9 有 6 条，推不出「39 个已给出结论」；实测 ≥400 非空行的 Rust 文件 **42 个**、前端 **9 个**（原文 41 / 8） |
| F4 | 命令清单可用，但缺记录载体与执行顺序约束；`npm run lint` 内含 `format`，会改写工作区 |

以下各节已按复核结果改写。

---

## 1 未完成项

### F1 Rust 两项拆分（归档遗留，非当前最大风险）

| 文件 | 非空行 | 变化原因 | 目标结构 | 前置条件（复核后） |
|---|---|---|---|---|
| `assistant-agent/src/multimodal.rs` | 692 | 配置与信任级别 / 校验 / 准入 / 重水化 / 投影 / 文本回退 | `multimodal/{mod,validate,blocks,admit,rehydrate,project,text}.rs` | 仅 `rehydrate_images` 无覆盖；其余路径已有 ≥17 个 `#[tokio::test]`（`tests/attachment_budget.rs`，其中 `project_files` 8 处、`admit_attachments` 5 处直接调用） |
| `assistant-providers/src/openai/responses_stream.rs` | 688（743 含空行） | 重试循环 / SSE 事件分支 / 工具调用累积 / 推理摘要分段 | `responses_stream/{mod,state,event_handler,in_progress,helpers}.rs` | 无流式与重试 fixture（providers 的 `tests/` 只有 `base` / `diagnostics` / `quirks`，文件内无 `#[cfg(test)]`）；主体是单函数内 `'retry: loop` + `async_stream::stream!`，探针缩进 13 层（全仓 Rust 最深） |

**F1-a 已完成（2026-11-17）**：

| 项 | 结果 |
|---|---|
| 前置用例 | `tests/attachment_budget.rs` 新增 `rehydrate_images_restores_cached_data_and_degrades_missing_ids`，覆盖「回填 `cached_data`」与「缺失 id 降级为 `[Image not found]`」两条分支；先跑该用例单独通过（1 passed） |
| 拆分结果 | `multimodal.rs`（692 非空行）→ `multimodal/{mod,validate,blocks,admit,rehydrate,project,text}.rs`；`mod.rs` 45 非空行，只留类型 / 常量 / 模块声明 / `pub use`；最大子模块 `text.rs` 184 |
| 变化原因落点 | `validate` = 信任级别配置 + 扩展名与路径校验；`blocks` = 消息块读写；`admit` = 准入与落盘；`rehydrate` = 图片回填；`project` = 预算分配与投影；`text` = 正文解码 / 截断 / 渲染与占位 |
| 对外 API | 零变更：`AttachmentLimits` / `ATTACHMENT_LIMITS` / `MAX_UNKNOWN_WINDOW_FILE_TOKENS` / `ProjectionPurpose` / `MultimodalConfig` / `admit_attachments` / `rehydrate_images` / `project_images_for_text_model` / `project_files` / `text_only_image_text` / `offloaded_image_text` / `file_handle_text` 全部保持原路径 |
| 验证 | `cargo clippy -p assistant-agent --all-targets -- -D warnings` 全绿；`cargo test -p assistant-agent` 全绿（lib 36 + attachment_budget 19 等）；`cargo check --all-targets` 全绿 |

动作要求（仅 F1-b 仍适用，F1-a 已收口）：

1. 先补缺口 fixture，再动结构。`responses_stream` 需覆盖 SSE 增量、`response.output_item.done` 与重试分支；crate 内无 HTTP mock 设施（`tests/` 只有 `base` / `diagnostics` / `quirks`，dev-dependencies 只有 `proptest`），因此 fixture 只能喂给「事件 JSON → 状态迁移」的纯函数，不引入 mock server。
2. 拆分按原则 §5「宏边界不可搬移」执行：状态与分支抽成返回值的纯函数，宏内只保留 `for ... { yield }`；`mod.rs` 按 §4.1 只放类型、常量、模块声明、组合入口与 `pub use`。
3. 纯搬移与语义改写分开提交，便于回滚。
4. 对外 API 零变更：`responses_stream` 的模块路径与导出集合保持不变。
5. 完成后在 `src-tauri` 目录跑 `cargo check` / `cargo clippy --all-targets -- -D warnings` / `cargo test`。

**优先级定位**：`responses_stream.rs`（688）在 900 行优先审查线以下，且 §2 已明确「行数不作拆分义务」。它是归档规格的遗留项、价值中等，**不是**「唯一剩余的代码工作」；当前真实的结构风险在 F3 的组 A。

**前置门（2026-11-17 复核新增）**：该文件对应的 `ProviderKind::OpenAIResponses` 无前端正式入口——`ApiFormat` 只有 `openai` / `anthropic` / `local`，仅「自定义 Provider ID」自由文本手输可达，且 I9 规格（已归档）H2 运行验证 8 项全未勾选。处置决策与影响面见 [responses-endpoint-disposition.md](./responses-endpoint-disposition.md)：选 A（补入口）则先补后拆，选 B（保留记录）则维持低优先级，选 C（移除）则 F1-b 作废转删除任务。决策落地前不动 `responses_stream.rs`。

**前置门结论（2026-11-17）**：已选 **A**，前端 `responses` / `gemini` format 与 preset 已落地（后端零改动）。但 [responses-endpoint-disposition.md](./responses-endpoint-disposition.md) §5.3 的 4 项运行时验收仍未执行，F1-b 维持**暂缓**——拆分前提「先补 fixture」的收益在验收前无法兑现。恢复条件：运行验收有结论，且确认该端点长期保留。

### F2 `crate-architecture.md` 口径统一（已收口）

口径与非空行数字已随本次结构文档批提交收口（2026-11-17）：主 app 116 文件 / 16,607 非空行，12 个子 crate 338 / 55,327，合计 454 / 71,934（`probe-structure.mjs` 实测，含 F3-A、F1-a 与 F3-C 的拆分）。后续「重刷 `crate-architecture.md` 的 §1.1 / §3.1 / 逐 crate 行数」固定为**结构变更批提交前的随批动作**，不再当作一次性待办；纯功能批不追数字。重刷时同步补齐 B2 / B4 的实际落点分组（`runtime/`、`os/tools/`、`os/shared/`、`kernel/turn/`、`storage/tools/`、`vault/security_db/{state,session,anchor}`）。

### F3 ≥400 非空行的账目重建（原账目不可用）

实测（probe 同口径，非测试、非生成物）：Rust **42 个**、前端 **9 个**。按「是否已有结论」分成六组，合计 42：

| 组 | 数量 | 判定状态 |
|---|---|---|
| A `mod.rs` 内含实现 | 2 | **已拆分**（`git_sync` / `runtime/compact`） |
| A′ `mod.rs` 实现合规、内联测试超标 | 4 | 判定「不拆实现」，见 F3-G |
| B 装配型 `mod.rs` / 状态集中 | 2 | 可判「保留」，需确认 |
| C 新增功能产出（I35 截图 / I53 沙箱） | 5 | I35 2 个**已判定并执行**（`screenshot/mod.rs` 与 `orb_overlay.rs`，见 F3-C）；I53 3 个待判定（前端 `Overlay.tsx` 在 F3-E，已判定） |
| D 存量未判定 | 21 | 待判定 |
| 已判定（F1 两个 + 归档 §2.9 六个） | 8 | 已判定 |

> 这 42 是「复核当时的账目」。已拆掉的 `git_sync/mod.rs`、`runtime/compact/mod.rs`、`multimodal.rs`、`screenshot/mod.rs`、`orb_overlay.rs` 五个不再 ≥400，故**现存 ≥400 的 Rust 文件为 37**（probe 同口径：A′4 + B2 + C3 + D21 + §2.9 六个 + `responses_stream.rs`）。F1-b 完成后为 36。归档 §2.9 六个即 `test_doubles.rs`（801，`#[cfg(any(test, feature = "test-doubles"))]` 门控的测试替身）、`middleware.rs`（524）、`db.rs`（583）、`policy.rs`（458）、`os/shared/output.rs`（479）、`os/shared/text_encoding.rs`（455）。

#### F3-A `mod.rs` 内含实现（复核修正：实际只有 2 个，且已完成）

> 2026-11-17 执行时复核：原判「6 个 `mod.rs` 内含实现」是把内联测试的 `fn` 算进了实现。以 `#[cfg(test)]` 为界重新统计后，**只有 2 个属实现回潮**，其余 4 个 `mod.rs` 实现部分仅 12–56 非空行。

| 文件 | 非空行 | `#[cfg(test)]` 前非空行 | 前段 `fn` | 结论 |
|---|---|---|---|---|
| `assistant-sync/src/git_sync/mod.rs` | 403 | 403（无内联测试） | 37 | **已拆** |
| `assistant-tools/src/runtime/compact/mod.rs` | 492 | 140 | 9 | **已拆** |
| `assistant-tools/src/os/shared/path/mod.rs` | 549 | 56 | 4 | 合规，超长来自内联测试 |
| `assistant-tools/src/mcp/stdio/mod.rs` | 482 | 26 | 0 | 合规，超长来自内联测试 |
| `assistant-tools/src/sandbox/windows/mod.rs` | 759 | 26 | 0 | 合规，超长来自内联测试 |
| `assistant-tools/src/mcp/bridge/mod.rs` | 405 | 12 | 0 | 合规，超长来自内联测试 |

已完成的两项（纯搬移，对外 API 零变更）：

| 文件 | 动作 | 目标结构 | 验证 |
|---|---|---|---|
| `assistant-sync/src/git_sync/mod.rs` | 403 → 18 非空行，按「状态 / 生命周期 / 远端 / 提交 / 冲突 / 纯函数」六分 | `git_sync/{mod,state,lifecycle,remote,commit,conflict,helpers}.rs` | `cargo test -p assistant-sync --features git-tests`：19 passed |
| `assistant-tools/src/runtime/compact/mod.rs` | 140 行实现下沉，`mod.rs` 只留模块声明与 `pub use` | `compact/{mod,envelope,error_kind,access,text}.rs` | `cargo test -p assistant-tools`：全绿 |

#### F3-B 装配型 `mod.rs` / 状态集中（2 个）

| 文件 | 非空行 | 现状 |
|---|---|---|
| `assistant-agent/src/agent/loop_executor/mod.rs` | 432 | 7 个 `fn` + 2 个 `impl` + 5 个 `mod`，符合 §4.1 的组合入口定位，倾向「保留」 |
| `assistant-vault/src/security_db/state.rs` | 472 | 33 个 `fn`、无 `mod`；是 §5「状态集中」的产物，需确认是否只承载单一状态职责 |

#### F3-C 新增功能产出（5 个：I35 已判定、I53 待判定）

| 文件 | 非空行 | 来源 | 结论 |
|---|---|---|---|
| `src-tauri/src/screenshot/orb_overlay.rs` | 720（拆分前） | I35 截图 | **已完成**（拆为 `orb_overlay/{mod,snapshot,platform,menu_fade}`；见 F3-C 判定） |
| `assistant-tools/src/sandbox/macos.rs` | 462 | I53 沙箱 | 待判定 |
| `assistant-tools/src/sandbox/linux.rs` | 440 | I53 沙箱 | 待判定 |
| `src-tauri/src/screenshot/mod.rs` | 34（拆分前 436） | I35 截图 | **已完成**（`mod.rs` 只留声明、错误码常量与 `pub use`；见 F3-C 判定） |
| `assistant-tools/src/sandbox/mod.rs` | 428 | I53 沙箱 | 待判定 |

这 5 个文件在功能开发中产生，既不在归档 §2.7 / §2.8 的清单里，也没有在本文登记过结论（2026-11-17 已全部登记，见下表与本节判定）。

I35 三个文件的判定（2026-11-17，随 I35 截屏提问规格（已归档） rev11 收口登记）：

| 文件 | 实现/测试非空行 | 变化原因 | 判定 |
|---|---|---|---|
| `src-tauri/src/screenshot/orb_overlay.rs` | 实现 ~620 / 测试 ~100 | ① Orb 窗口快照-隐藏-全屏-恢复编排；② Windows 弹出菜单淡出排空（DWM 帧 + `#32768` 枚举，约 300 行）与其它平台 stub | **拆分**：`orb_overlay/{mod,snapshot,platform,menu_fade}`。理由是两个变化原因确实独立——菜单淡出只在 Windows 生效且以平台 `mod` 分叉，改它不应触碰几何/可见性编排。`platform` 单独成目录：DWM 过渡禁用 / `ShowWindow` 兜底 / `DwmFlush` 是 Tauri 未暴露的原生原语，被 `snapshot` 与 `menu_fade` 共同调用。搬移为纯搬移，对外 API（`begin`/`present`/`exit`/`recover_leftover`/`dismiss_menus_now`/`SCREENSHOT_ACTIVE`）零变更 |
| `src-tauri/src/screenshot/mod.rs` | 实现 ~415 / 测试 ~20 | `mod.rs` 内含状态机实现（`start` 130 行、`crop` 100 行、清理与相位辅助），违反「`mod.rs` 只做声明与 `pub use`」 | **拆分**：`screenshot/{mod,state,start,crop_command,cleanup}.rs`；`mod.rs` 只留 `pub mod` 声明、错误码常量与 `pub use`。`crop_command` 而非 `crop`：`screenshot/crop.rs` 已是几何换算 + 编码模块，两者不能同名 |
| `src/screenshot/Overlay.tsx` | 427 | 单一职责：覆盖层交互（指针拖拽/8 向手柄 + 取消/确认副作用 + 渲染）。纯函数已外移到 `selection.ts`，会话逻辑已外移到 `useScreenshotSession.ts` | **保留**：未出现第二个变化原因，无职责混杂。约束：M3 标注能力落地前必须先抽出 `useSelectionDrag`，否则标注与框选会在同一文件叠加第二个变化原因 |

两个 Rust 文件按「纯搬移 + 对外 API 零变更 + 单测随文件移动」三条件执行。`screenshot/mod.rs` 已于 2026-11-17 执行完毕：落点 `mod`(34) / `state`(137) / `start`(138) / `crop_command`(97) / `cleanup`(88)，3 个内联测试随职责移到 `state` 与 `cleanup`。`orb_overlay.rs`（750 非空行）同批执行完毕：落点 `orb_overlay/mod`(133) / `snapshot`(374，含 8 个内联测试) / `platform/{mod,windows,fallback}`(9/71/14) / `menu_fade/{mod,windows,fallback}`(11/212/12)，7 个内联测试随职责移到 `snapshot`。同批把 `capture.rs` 的 macOS 授权门控（`#[cfg]` 内联模块 + 两个平铺函数）拆为 `capture/permission/{mod,macos,fallback}`，`capture` 由单文件改为目录（`capture/mod.rs` 295 非空行）；平台差异一律「每平台一个实现模块 + 分派 `mod.rs` 只做 `pub use`」，不再逐函数 `#[cfg]` 平铺。`cargo test --lib screenshot` 21 passed、`clippy --all-targets -D warnings` 无告警。

#### F3-D 存量未判定（21 个）

| 文件 | 非空行 |
|---|---|
| `src-tauri/src/protocol/dto/agent.rs` | 585 |
| `src-tauri/src/cmd/chat/insert.rs` | 569 |
| `src-tauri/src/window.rs` | 524 |
| `assistant-tools/src/os/tools/agent_ask_user.rs` | 495 |
| `assistant-storage/src/conversations/msg_ops.rs` | 492 |
| `assistant-agent/src/agent/loop_executor/terminal.rs` | 473 |
| `src-tauri/src/tool_setup.rs` | 459 |
| `assistant-crypto/src/vault_crypto/mod.rs` | 457 |
| `assistant-storage/src/conversations/ops.rs` | 451 |
| `assistant-providers/src/gemini/message_converter/messages.rs` | 439 |
| `assistant-vault/src/lifecycle.rs` | 438 |
| `src-tauri/src/adapters/question/registry.rs` | 434 |
| `assistant-providers/src/anthropic/message_converter/messages.rs` | 433 |
| `assistant-providers/src/openai/message_converter.rs` | 431 |
| `src-tauri/src/adapters/approval.rs` | 424 |
| `assistant-providers/src/client/quirks.rs` | 417 |
| `src-tauri/src/cmd/vault/entries.rs` | 413 |
| `src-tauri/src/cmd/vault/sync.rs` | 410 |
| `src-tauri/src/cmd/vault/mod.rs` | 410 |
| `assistant-tools/src/os/tools/agent_read/window.rs` | 407 |
| `assistant-providers/src/anthropic/request.rs` | 405 |

#### F3-E 前端 ≥400 非空行（9 个）

| 文件 | 非空行 | 状态 |
|---|---|---|
| `src/tools/password-generator/effWordlist.data.ts` | 2,026 | 不动（数据表，算法已外移） |
| `src/services/stream/turnOrchestrator.ts` | 504 | 不动（归档 §2.8 判定） |
| `src/vault/core/slices/vaultSyncSlice.ts` | 493 | 不动（状态切片） |
| `src/chat/ConversationView.tsx` | 457 | 不动（装配点） |
| `src/stores/chat/chatConversationSlice.ts` | 454 | 不动（状态切片） |
| `src/settings/core/runtime/snapshot.ts` | 438 | 不动（单一快照契约流水线） |
| `src/vault/detail/VaultEntryDetail.tsx` | 429 | 不动（未出现第二个变化原因） |
| `src/screenshot/Overlay.tsx` | 427 | 保留（I35，判定见 F3-C） |
| `src/orb/OrbDrag.tsx` | 402 | 不动（未出现第二个变化原因） |

#### F3-F 归档规格两张清单的失效映射（保留为历史记录）

| 清单中的路径 | 现在的位置 |
|---|---|
| `assistant-tools/src/gateway.rs` | `gateway/{mod,approval_key,context,decision,stores,testing}.rs` |
| `assistant-tools/src/os/path.rs` | `os/shared/path/{mod,args,membership,protected,resolve}.rs` |
| `assistant-tools/src/mcp/stdio.rs` | `mcp/stdio/{mod,client,transport,codec,stderr}.rs` |
| `assistant-tools/src/compact.rs` | `runtime/compact/{mod,types,head,anchor,truncation,preview}.rs` |
| `assistant-app` `cmd/conversation.rs` | `cmd/conversation/{mod,list,detail,detail_dto,mutate,branch,export}.rs` |
| `assistant-vault/src/security_db/mod.rs` | `security_db/{mod,state,session,anchor}.rs` |
| `src/settings/shared/sections/PermissionAuditSection.tsx` | `PermissionAuditSection/{index,range,columns,format,stats,useColumnResize,AuditTable}.ts(x)` |
| `src/vault/settings/VaultSyncSettings.tsx` | `VaultSyncSettings/{index,syncResult,statusAlert,SyncNotices,RemoteConfigForm,SyncActions}.ts(x)` |
| `src/mascot/renderers/svg/GhostyMascot.tsx` | `svg/GhostyMascot.tsx` + `svg/{facePresets,mouthPaths,useGhostyAnimation}.ts(x)` |
| `src/settings/provider/ConnectDialog.tsx` | `ConnectDialog/{index,fieldSchemas,useConnectForm,ModelPreviewList}.ts(x)` |

结论：两张清单只能当「已完成记录」读，不能当待办读。归档 §2.7 的 21 条中只有 `multimodal.rs` 与 `responses_stream.rs` 仍指向现存文件，其余路径已被 B2 / B4 拆掉（`multimodal.rs` 已于 F1-a 拆完，仅剩 `responses_stream.rs`）。

#### F3-G 内联测试超标（4 个，判定「暂不执行」）

`os/shared/path/mod.rs`（493 行测试）、`mcp/stdio/mod.rs`（456）、`mcp/bridge/mod.rs`（393）、`sandbox/windows/mod.rs`（733）的实现部分都合规，文件超 400 行完全来自内联测试。原则 §3 规定的动作是**把测试外移、实现文件不动**：

- 被测项是公开 API 的（`mcp/bridge`、`mcp/stdio`）→ 移到 `crates/assistant-tools/tests/<主题>.rs`；
- 被测项非公开的（`sandbox/windows` 的 `acl`、`os/shared/path` 的 `join_input` / `simplify_verbatim`）→ 按 crate 既有先例加 `internal-tests` feature（`assistant-providers` 已在用 `#[cfg(all(test, feature = "internal-tests"))]`）。

判定为**暂不执行**，理由：这些文件没有职责混杂，移测试只改可导航性、不改结构判据；`sandbox/windows` 的 733 行测试是本机唯一的 Windows 沙箱行为证据，搬移会牵动 `internal-tests` 门禁与特性组合。等 F3-B / C / D 判定收口、且确有可导航性诉求时再执行。

### F4 归档后的全量验收

归档规格 B4 记录中的「全部通过」是搬移当时的结果。冻结结构基线前应补一次干净的全量证据：

```
# Rust（工作目录 src-tauri）
cargo check
cargo clippy --all-targets -- -D warnings
cargo test

# 前端（仓库根）
npm run typecheck
npm run lint
npm test

# 契约（仅当 DTO / 配置组有改动时）
npm run contract:check
```

约束（复核后补充）：

1. **记录载体**：结果登记在本节表格（日期 / 命令 / 结果），或写进提交信息；二选一，不新建文档。
2. **顺序**：`npm run lint` 内含 `npm run format`（会改写工作区），因此先 `format` 后 `eslint`，`typecheck` 在 lint 之后跑，避免格式改写引入的类型问题被漏掉。
3. **干净度**：验收须在无未提交改动的状态跑；若工作区在飞（如截图功能未提交），结论只能标为「工作区验收」，不能当作基线。

| 日期 | 命令 | 结果 |
|---|---|---|
| 待补 | — | — |

---

## 2 归档时确立的口径（后续判断沿用，不要重新发明）

| 项 | 口径 | 依据 |
|---|---|---|
| 目录深度 | **观察信号，不设上限**。判据是「每个非生成目录对应一个可命名的变化原因」与「无转发层」，不是段数 | 路径段数不等于语义层数；组件 parts 桶、生成物目录占段不占层 |
| 扁平度 | 直接子模块 ≥12 **只是审查触发线**，不是分组义务；分组条件是存在 ≥2 个独立变化原因的子域且各 ≥3 文件；uniform peers 永远保持扁平 | 参考项目中 `codex core/src` 根目录平铺 138 个文件，说明数量线无上游依据 |
| 职责清晰 | 唯一判据是一个文件对应一个变化原因；行数只作初筛（≥900 优先审查），不作拆分义务与验收标准 | [CODE-SPLIT-PRINCIPLE.md](../principle/CODE-SPLIT-PRINCIPLE.md) §1–§2 |
| `mod.rs` 内容 | 只放类型定义、共享常量、模块声明、组合入口与对外 `pub use`；`mod.rs` 内出现成规模实现即属回潮，与总行数无关 | 同上 §4.1、§6 |
| 新增大文件 | 功能开发新增 ≥400 非空行文件时，必须在功能验收时给出结论（保留 / 拆分 / 待判）并登记到 F3 账目；未登记即视为「待判」 | I35 的 3 个已于 2026-11-17 判定并登记（F3-C 两个均已执行、F3-E 一个保留）；仍未判定的只剩 I53 的 `sandbox/{macos,linux,mod}.rs` |
| 结构数字 | 只认 `node scripts/probe-structure.mjs`；文档内不得维护独立统计命令 | 避免口径漂移 |
| 数字时刻 | probe 统计含未跟踪与未提交改动，跨批比较只作量级参考；结构数字在结构变更批提交时同批重刷 | 复核当天数分钟内 `orb_overlay.rs` 从 674 变为 720 非空行 |

---

## 3 执行顺序

按「风险 × 机械程度」排序，前两项可独立提交、互不阻塞：

| 序 | 事项 | 动作要点 | 状态 |
|---|---|---|---|
| 1 | F3-A 两项真实回潮 | 纯机械搬移：`mod.rs` 只留声明与组合入口，实现下沉为具名子模块；对外路径不变 | **已完成**（`git_sync` 403→18、`runtime/compact` 492→声明+`pub use`；clippy 与测试全绿） |
| 2 | F1-a `multimodal.rs` | 先补 `rehydrate_images` 一条用例，再按六个变化原因拆成目录模块 | **已完成**（用例先过；`mod.rs` 45 非空行，clippy 与测试全绿） |
| 3 | F3-C-a `screenshot/mod.rs` | 纯搬移：状态机实现下沉为 `state` / `start` / `crop_command` / `cleanup`，`mod.rs` 只留声明、错误码常量与 `pub use` | **已完成**（2026-11-17，`mod.rs` 436→34 非空行；20 个单测随职责落位，clippy 与测试全绿） |
| 4 | F3-C-b `orb_overlay.rs` | 纯搬移：拆 `mod` / `snapshot` / `platform` / `menu_fade`，对外 API 不变 | **已完成**（2026-11-17，`orb_overlay.rs` 750→`mod`(133) / `snapshot`(374) / `platform`(94) / `menu_fade`(235)；同批把 `capture.rs` 的 macOS 授权门控拆为 `capture/permission/{mod,macos,fallback}`；21 passed、clippy 无告警） |
| 5 | F1-b `responses_stream.rs` | 先补 SSE 增量 / 工具完成 / 重试三类 fixture，再按 §5 抽纯函数，宏内留 `yield` 循环 | 暂缓（前置门结论为 A 已落地，等 §5.3 运行验收；688 行未达 900 拆分线） |
| 6 | F3-B / D 判定 + I53 判定 | 逐个给结论；判「不拆」的写明唯一变化原因，判「拆」的登记目标结构 | 部分完成（I35 的 3 个已判定并登记，见 F3-C / F3-E）；I53 3 个与 F3-B / D 待开始 |
| 7 | F3-G 内联测试外移 | 4 个文件按 §3 移测试；已判定「暂不执行」 | 暂不执行 |
| 8 | F2 重刷 + F4 验收 | 结构变更批提交前同批跑 probe 重刷 `crate-architecture.md`，再跑 F4 全量 | 随结构变更批 |

---

## 4 复现

```
node scripts/probe-structure.mjs          # 全量（Rust + 前端），输出 temp-backup/structure-probe.md
node scripts/probe-structure.mjs rust     # 仅 Rust workspace
```
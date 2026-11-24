# I72 — 审批卡片决策化任务清单

| 项 | 值 |
| --- | --- |
| 状态 | 任务清单（唯一任务级进度；进度只在本文件勾选） |
| 日期 | 2026-11-24 |
| 设计 | [I72-approval-card-decision-design.md](./I72-approval-card-decision-design.md)（唯一设计来源；本文只列改动点与验收入口，不复述规则） |
| 判据 | 设计 §11：A1–A13f / B14–B18d / C19 / D20–D21c / E22–E28 |
| 前置 | I38d（已归档，四动作硬约束）、I70、I71 已完成 |

批次：W1 视图模型与字典（§5）→ W2 卡片结构改版（§3/§4/§7/§8.2/§8.4）→ W3 拒绝理由（§8.5）→ W4 后端与 wire 结构化（§10.2 + §2.5 + §6.7）→ W5 验收清理。

实现顺序：**4b 先改 wire 并跑 `contract:gen`**（生成类型是所有消费方的前置）→ 4a 与 W1 并行 → W2 → W3 → W5。五个批次同批交付。

> 本清单不管理提交，只回答「代码改没改、验收跑没跑」。勾选表示代码已改且对应验收已跑。

---

## W1 — 视图模型与字典（§5，A1–A13f 中归纯函数的部分）

- [x] `src/chat/tool/approvalPresentation/`（新增目录模块，按 `docs/principle/CODE-SPLIT-PRINCIPLE.md` 按职责拆分；对外入口 `index.ts`）：`ApprovalView` 类型与 `buildApprovalView`，按 §5.1 顺序求值，空要素返回 `null`；按 §7.2 优先级产出 `reasons.primary` / `reasons.rest` 两个数据槽，不产出文案；不 import React、不按 `toolName` 分支、不接收工作区根（N18；签名断言见 B18）
  - `types.ts` 视图契约｜`view.ts` 组合入口｜`elements.ts` §5.1 要素求值 + 取词｜`evidence.ts` §5.3 证据块｜`diagnostics.ts` N7 诊断与降级｜`detail.ts` §7.2 原因 / §8.2 授权范围 / §4.5 区块顺序｜`text.ts` 叶子纯函数
- [x] `approvalPresentation/tables.ts` 纯表：参数语义字典（§5.2）、`TOOL_PROFILES` 与 `TOOLBOX_ONLY_IDS`（§5.5）、`EXTENSION_KIND`（`kind.code` 复合键，§3.1）、`INTENT_ACTION`（§8.2，含 `readToolOutput`）
- [x] `approvalPresentation/elements.ts` / `evidence.ts` / `diagnostics.ts`：证据形态（§5.3）、`summary` 为空即不渲染（含 `truncated` / `estimated` 无正文不标注，§5.3）、消费 W4 产出的结构化字段（`verbCode` / `entries` / `bytes` / `targetKind` / `exists` / `symlink` / shell 结构）
- [x] `src/chat/tool/toolCallDisplay.ts`：新增 `formatAction`，从 `approvalPresentation/tables.ts` 的 `INTENT_ACTION` 取词（不另建表）；末段复用 `workspaceBasename`（N18）
- [x] `src/i18n/locales/zh-CN.json` / `en-US.json`：§10.1 列出的全部新键，双语对齐；移除 `always_hint`（须与 W2 删除 `ApprovalCard` 的引用同批，单独先删会让 `typecheck` 失败）
- [x] `src/chat/tool/__tests__/approvalPresentation.test.ts`（新增）：A1–A8、A9b、A10b–A10f、A12、A13–A13f（A9 只测首屏、A13b 只测键集合）；`TOOL_PROFILES` 键集合 = 源码扫描 `TOOL_ID` 扣除 `TOOLBOX_ONLY_IDS`。A11（`reasonText` 六分支）与 A13e 的签名断言不在此文件（§11）

> 验收：`npx vitest run src/chat/tool/__tests__/approvalPresentation.test.ts`；`npm run typecheck`。

---

## W2 — 卡片结构改版（§3/§4/§7/§8.2/§8.4，B14–B18、C19）

- [x] `src/chat/tool/ApprovalCard.tsx`：改为四区块（决策句 / 证据块 / 按钮 / 折叠区），删 `row` 平铺与 `preview` / `intents` / `impact` / `extensions` / `reasons` 原始渲染；折叠区按 §4.5 固定顺序（原因 → 授权范围 → 证据 → 全部参数 → 诊断 → 拒绝后果），入口文案按实际区块拼接
- [x] 同文件：范围说明（§8.2，删 `alwaysHint` 改按需展示）、`always_confirm` 内容扩充（§8.3）、折叠区末尾拒绝后果一句（§8.4）
- [x] `src/chat/tool/ToolCallBubble.tsx`：`awaiting_user` 态隐藏上方 `name + argsSummary` 行
- [x] `src/chat/tool/ApprovalCard.module.css`：新增 `decision` / `commandBlock` / `boundary` / `evidence` / `fold`；删 `row` / `label` / `intent` / `extension` / `alwaysHint` 平铺样式
- [x] `src/chat/tool/__tests__/approvalRulesUi.test.ts`：追加 B14–B18 结构断言与 C19 双语键对齐；既有六种 `reason` 分支断言保持通过（A11，不新增）

> 验收：`npx vitest run src/chat/tool/__tests__/`；`npm run typecheck`。

---

## W3 — 拒绝理由（§8.5，E22–E28）

- [x] `src/chat/tool/ApprovalCard.tsx`：`decide` 增可选 `reason`，仅 `deny` 且 `trim()` 非空时带字段，其余省略（N17）
- [x] 同文件：`denyReasonOpen` 与弱化文本入口（默认不渲染输入框）；展开态单行输入（`maxLength=200`、自动聚焦、回车不提交）；提交失败保留展开与已输入内容；超时态随按钮一并隐藏/置灰
- [x] `src/chat/tool/ApprovalCard.module.css`：新增 `denyReasonEntry` / `denyReasonField`，不改动作行高度与按钮顺序
- [x] `src/i18n/locales/zh-CN.json` / `en-US.json`：`deny_reason_open` / `deny_reason_close` / `deny_reason_placeholder` / `deny_reason_hint`
- [x] `src/chat/tool/__tests__/approvalRulesUi.test.ts`：追加 E22–E28 源码断言

> 验收：`npx vitest run src/chat/tool/__tests__/approvalRulesUi.test.ts`；`npm run typecheck`；手工 E22–E28。

---

## W4 — 后端与 wire 结构化（§10.2 + §2.5 + §6.7）

**4a 不改 wire**

- [x] `src-tauri/crates/assistant-tools/src/os/tools/agent_delete/mod.rs`：`validate_input` 增「`recursive=false` 且目标为非空目录 → invalid」检查（复用 `fs_util::ensure_empty_directory`）；`trash.rs` / `exec.rs` 原检查留作 TOCTOU 兜底
- [x] `src-tauri/crates/assistant-tools/src/runtime/registry.rs`：新增 `find_for_agent(name)`（过滤 `audience().agent`）
- [x] `src-tauri/crates/assistant-tools/src/runtime/pipeline.rs`：**三处** `find_by_name` 改 `find_for_agent`——批次切分、`inline_output_limit` 落盘阈值回查、`run_call` 的 Find 阶段
- [x] `src-tauri/src/adapters/tool_escalation.rs`：`resolve` 改 `find_for_agent`，去掉对 `find` 的回落
- [x] `src-tauri/crates/assistant-tools/src/os/mod.rs`：`register_os_tools` → `register_bridge_tools`，调用点同步
- [x] `src-tauri/crates/assistant-tools/src/runtime/gateway/context.rs`：`workspace_extensions` 只对路径型 action（`read` / `readToolOutput` / `edit` / `delete` / `purge`）判定，`execute` / `mcp` 不产出 `workspace` 条目；删除 `output_extensions` 与 `writable_root_name`（`sandbox.root` / `output.scope` 产出点整体移除，§3.1）
- [x] 守卫测试：`tests/registration.rs`（B18b）、`tests/pipeline.rs`（B18c）、`context.rs` 单测（B18d）

**4b 改 wire（`contract:gen`）**

- [x] `src-tauri/crates/assistant-tools/src/contract/trait.rs`：`ApprovalPreview` 增结构化字段（`entries` / `bytes` / `target_kind` / `exists` / `symlink` / `verb_code` 与 shell 结构）
- [x] `src-tauri/crates/assistant-protocol/src/agent.rs`：`ApprovalPreviewDto` 同步增字段；`ApprovalExtensionDto` 收敛为 `kind + code + severity`（**删 `label` / `value`**）
- [x] `src-tauri/crates/assistant-tools/src/runtime/gateway/context.rs`：`ApprovalExtension` 同步收敛为 `kind + code + severity`，`sandbox_extensions` 的 `code` 取 `full` / `partial`，`workspace_extensions` 取 `inside` / `outside` / `partial`（§3.1）
- [x] `src-tauri/crates/assistant-tools/src/dto_bridge.rs`：`From<ApprovalPreview>` 与 `From<ApprovalExtension>` 同步
- [x] 填充 `preview_for_approval`：`agent_delete/scan.rs`（规模统计复用 `scan.rs`）、`agent_write/preview.rs`、`agent_edit/report.rs`、`super_edit/mod.rs`、`agent_shell/mod.rs`；`agent_mkdir.rs` **新增**该实现（`verb_code = "mkdir"`）
- [x] `src-tauri/crates/assistant-tools/src/os/tools/agent_shell/`：新增语义解析器（S1–S4，必须接受 `ShellFlavor`），`CommandClass` 四值输出不变；**删除 `summary_for`**，`summary` 置空串（§5.3、§10.2）
- [x] 运行 `npm run contract:gen`，提交 `src/types/protocol.contract.ts` 与 `src/vault/core/contract/vault.contract.ts`；禁止手改生成类型
- [x] `tests/agent_delete.rs` / `agent_write.rs` / `agent_shell.rs` / `agent_mkdir.rs`：新增结构化字段断言

> 验收：`cargo test -p assistant-tools`（cwd = `src-tauri`，至少覆盖 `agent_delete` / `agent_write` / `agent_shell` / `agent_mkdir` / `registration` / `pipeline`）；`npm run contract:gen` 后 `npm run typecheck`。

---

## W5 — 验收与清理

- [x] `npm run lint:fix`（先直接修，不再重复跑 lint 检查）
- [x] `npm run typecheck`
- [x] `npm run test`
- [x] `cargo test -p assistant-tools`（cwd = `src-tauri`）
- [ ] 手工 D20 / D21 / D21b / D21c
- [x] 复核 §12 不做项未被引入（无模板分支、无前端重算策略结论、无 `TOOLBOX_ONLY` 工具进入 `TOOL_PROFILES`、无模型调用路径残留 `find_by_name` / `find`、无手改生成类型）
- [x] 复核本次收敛到位：`ApprovalExtensionDto` 无 `label` / `value`、无 `sandbox.root` / `output.scope` 产出点、无 `summary_for`、无范围说明常驻偏好项（§3.1、§5.3、§8.2、§12）

> 验收记录：2026-11-24；`npm run test`（95 文件 / 1153 通过 / 1 跳过）、`npm run typecheck`（通过）、`npm run lint:fix`（无残留告警）、`cargo test -p assistant-tools`（通过）；§12 与收敛复核用源码扫描确认：`find_by_name` 仅存于注册表定义、测试与 `tool_setup` 断言，模型调用路径（`pipeline.rs` / `tool_escalation.rs`）无残留；`src` 下 `always_hint` / `alwaysHint` 仅存于断言；`ApprovalExtensionDto` 为 `kind + code + severity`。异常说明：无。
# 超长文件拆分与职责归位规划

> 更新日期: 2026-11-01
> 范围: `src-tauri/` 下全部 `.rs`（排除 `tests/`、`examples/`、`target/`）
> 统计口径: 行数（非空行，含注释与内联测试）仅作**初筛参考**与横向对比，不用于判断拆分粒度，也不作为验收标准。文中出现的行数均为某一时点的快照，允许有偏差。
> 拆分判据: **职责边界**（一个文件只对应一个变化原因），与行数无关。拆分方案只给出子模块的职责与函数归属，不设"拆到多少行"的目标。
> 初筛参考: ≥900 行优先审查 / 300–899 行视职责数 / <300 行一般不动

---

## 0. 核心结论

1. **长度重心集中在 vault 与 security**：三份原 P0 超长文件（`assistant-vault/security_db/crud.rs`、`sync_engine/mod.rs`、`assistant-security/unlock_dialog/windows/mod.rs`）已按 Phase A / B / C 拆分完毕，当前 `src-tauri` 内 ≥900 行文件为 0，最长文件为 `openai/responses_stream.rs`（687，详见 1.2 与 4）。
2. **长度只是症状，职责混杂才是病因**：三份 P0 文件各自承载 5–6 类互不相关的职责（详见第 2 节），单纯按行数切片会把耦合切开却留下职责错位。
3. **优先级不应按行数排**：按"职责数 × 跨模块耦合度 × 改动频度"重排后，最该先拆的是 `security_db/crud.rs` 与 `cmd/conversation.rs`；`unlock_dialog/windows` 与 `openai/responses_stream` 属"高 `unsafe` 密度 + 需引入新状态结构"的真重构，风险大于收益（详见第 8 节）。前两者中 `security_db/crud.rs` 已完成，`unlock_dialog/windows` 亦已完成（未引入新状态结构，仅做职责搬移），`openai/responses_stream` 仍待评估。

---

## 1. 现状总览

### 1.1 按 crate

> 以下数字为 2026-11-01 快照，仅用于筛选优先级与横向对比，不承诺精确。

| crate | src 文件数 | 非空行（参考） | 最长文件 | 平均 |
|---|---|---|---|---|
| assistant-providers | 42 | 8,522 | `openai/responses_stream.rs` | 203 |
| src-tauri/src | 44 | 6,838 | `cmd/conversation.rs` | 155 |
| **assistant-vault** | 45 | **6,235** | **`dto/convert.rs`**（原 `security_db/crud.rs` 已拆） | 139 |
| assistant-security | 24 | 2,710 | `unlock_dialog/windows/wm_handlers.rs`（原 `windows/mod.rs` 已拆） | 113 |
| assistant-storage | 17 | 2,175 | `agent_modes.rs` | 128 |
| assistant-agent | 12 | 1,476 | `multimodal.rs` | 123 |
| assistant-sync | 5 | 1,139 | `git_sync/git.rs` | 228 |
| assistant-tools | 8 | 702 | `os/fs_link.rs` | 88 |
| assistant-config | 8 | 612 | — | 76 |
| assistant-crypto | 6 | 596 | — | 99 |
| assistant-orb | 4 | 362 | — | 90 |
| assistant-i18n | 1 | 124 | — | 124 |
| assistant-crash | 2 | 89 | — | 44 |
| `build.rs` | 1 | 60 | — | — |
| **合计** | **219** | **31,640** | — | 144 |

> `assistant-vault` 是当前最大风险面：它同时承载加密、SQLCipher、软删除、历史恢复、导入导出、同步编排与应急恢复。

### 1.2 初筛名单

> 分档只是"先看哪些文件"的参考，不构成拆分义务，也不是拆分目标。

| 区间（参考） | 文件数 | 文件 |
|---|---|---|
| ≥1000 | 0 | 原 `security_db/crud.rs`（1198）、`sync_engine/mod.rs`（1149）、`unlock_dialog/windows/mod.rs`（954）已按 3.1 / 3.2 / 3.3 拆分，本档现已清空 |
| 600–999 | 1 | `openai/responses_stream.rs` |
| 500–599 | 1 | `assistant-agent/multimodal.rs` |
| 400–499 | 9 | 见 3.6 P2 清单 |
| 300–399 | 21 | 7 个进 P2（见 3.6），11 个留观察（见 3.7），另 3 个为已拆子模块（`wm_handlers.rs` 352 / `history.rs` 327 / `subclass.rs` 303） |

---

## 2. 职责分化诊断

判据：**一个文件只应有一个变化原因**。若同一文件需要因"同步策略变更""加密算法变更""UI 布局变更"三类不同需求分别修改，即为职责混杂。

### 2.1 职责混杂清单（职责数 ≥3）

| 文件 | 混杂的职责 | 拆分主线 |
|---|---|---|
| `assistant-vault/security_db/crud.rs` ✅ | ① 公开查询（list/get/snapshot）② 写事务封装（`*_tx` + `with_transaction`）③ 软删除与历史恢复（restore/purge）④ 导入路径内部实现（`*_in` / `upsert_imported_*`）⑤ 快照读取 ⑥ 字段级加解密 | 读 / 写 / 事务 / 删除恢复 / 导入 / 快照 / 字段加密（已落地为 `security_db/entries/`，见 4） |
| `assistant-vault/sync_engine/mod.rs` ✅ | ① 同步编排（pull/push/sync_now）② Git 远端运维（init/clone/remote/discard）③ 历史与冲突预览（history/conflict preview/restore）④ 应急与灾难恢复（emergency/disaster export）⑤ 预览脱敏与路径 helper | 编排 / 远端运维 / 历史冲突 / 应急 / 预览 helper（已落地为 8 文件，见 3.2 与 4） |
| `assistant-security/unlock_dialog/windows/mod.rs` ✅ | ① Win32 FFI 声明 ② `DialogState` 与生命周期 ③ 输入框子类化（`edit_subclass_proc`）④ 按钮子类化与图标绘制 ⑤ 窗口过程 `wnd_proc` ⑥ 明文读取与安全清零 ⑦ 深色按钮自绘 | FFI / state / subclass / wnd_proc / wm_handlers / paint / text（已落地为 8 文件，见 3.3 与 4） |
| `assistant-providers/openai/responses_stream.rs` | 单一 `responses_stream()` 主函数承担全部职责：retry 循环、SSE 解析、事件分发、tool call 累积、delta 生成 | 入口 / 事件处理 / 累积状态 / helper |
| `assistant-agent/multimodal.rs` | ① 配置与信任级别 ② 路径校验与 MIME 推断 ③ 附件准入 ④ 图片/文件重水化 ⑤ 面向纯文本模型的投影 ⑥ 文本回退话术 | config / validate / admit / rehydrate / project / text |
| `src-tauri/src/cmd/conversation.rs` | 20 个 tauri command + DTO 组装（`build_detail_dto`）+ 三种分页变体 + 分支操作 | 列表 / 详情 / 变更 / 分支 / 导出 |
| `assistant-vault/dto/convert.rs` | 5 类共 23 个 `From` / `TryFrom` 实现（sync 15、totp 3、error 2、hibp 1、entry 2） | 按领域拆分子模块；`sync` 一类需再按 conflict / history / config / pending 细分 |
| `src-tauri/src/protocol/dto/chat.rs` | 会话 / 消息 / token 用量 / 事件载荷 / 附件引用 / 内容块 DTO | 按事件域拆 |
| `src-tauri/src/cmd/vault/sync.rs` | 20 个 command 覆盖同步、Git 远端、历史、冲突四类 | 同步 / Git / 历史 / 冲突 |
| `src-tauri/src/cmd/compaction.rs` | 2 个入口 command + `execute_compaction` 单函数（提示词构建、provider 调用、流消费、落库）+ 部分结果落库 | 入口 / 压缩 pipeline / 落库 |
| `assistant-providers/gemini/request.rs` | `chat_completion_stream` 单函数（请求体组装 + 发送 + 流包装）+ URL 推导 | 请求体构建 / 流执行 / URL |
| `assistant-security/screen_lock/mod.rs` | 三平台监听器同文件（windows / macos / linux） | 按平台分文件 |
| `assistant-tools/os/fs_link.rs` | 工具壳（schema/权限/tier）+ 路径校验 + 三种链接创建 + 链接检查 | 工具壳 / 平台实现 |
| `assistant-sync/git_sync/git.rs` | 44 个 git 命令封装：仓库、分支、远端、日志、底层 `Command` 拼装 | repo / branch / remote / log / plumbing |
| `assistant-providers/tool_call_accumulator.rs` | 累积器 + 截断 JSON 修复 + 大量内联测试 | 实现留在本文件，测试外移 |
| `assistant-providers/client/quirks.rs` | 厂商 URL 识别 + reasoning effort 归一 + SSE 错误提取 + 大量内联测试 | 实现留在本文件，测试外移 |
| `src-tauri/src/protocol/dto/mod.rs` | 仅 14 行 re-export，其余全部是 `#[cfg(test)]` round-trip 测试 | 测试外移到集成测试 |
| `assistant-vault/vault_file/mod.rs` | 条目文件读写 + vault_meta 签名/校验 + 应急数据 + 主密钥重写 | entry / meta / emergency |
| `assistant-agent/chat_runtime.rs` | 流结果类型 + 双累积器 + `consume_stream` + token 估算 | types / accumulator / consume |

### 2.2 三类跨层职责错位

1. **cmd 层承载业务编排**：`cmd/compaction.rs` 的 `execute_compaction` 承担了提示词构建、provider 调用、流消费、落库；`cmd/conversation.rs` 承担 DTO 组装。command 层应只做参数校验 + 调 domain + 发事件。
2. **内联测试挤占实现文件**：`protocol/dto/mod.rs` 的实现只有 14 行 re-export，其余全是测试；`providers` 的 `quirks.rs`、`tool_call_accumulator.rs`、`types/chat_delta.rs` 内联测试各占很大比重。这类文件"看起来超长"但职责并不混杂，动作应是移测试而非拆实现。
3. **平台相关代码未按平台分文件**：`screen_lock/mod.rs` 三平台同文件；`fs_link.rs` 多平台链接实现同文件。`unlock_dialog/` 已有 `linux.rs` / `macos.rs` / `windows/` 的范式，可直接对齐。

### 2.3 优先级判据

排序 = 职责数 × 跨模块耦合度 × 修改频度（行数仅作辅助权重，不单独决定优先级）：

- **P0**：职责 ≥5 类，或单文件承担完整子系统（加密存储、同步编排、原生 UI），或被多个上层模块调用。
- **P1**：职责 3–4 类，或单个函数承担完整流程（流式响应、附件准入）。
- **P2**：职责 ≥3 才拆；职责单一的只做测试外移或保持观察。

补充：`unlock_dialog/windows/mod.rs` 只有一个调用点、收益面窄，但整文件为 `unsafe` 原生窗口代码，风险维度独立，按 P0 归档、排最后执行（已完成，见 3.3）。

---

## 3. 拆分方案

> 以下"现状"按**职责段**列出函数归属，不使用行号区间；拆分后的子模块只列职责与函数，不估算行数。

### 3.1 P0-1 `assistant-vault/security_db/crud.rs`

现状（按职责）：

| 职责段 | 内容 |
|---|---|
| imports | 模块引入 |
| 公开查询 | `list_entries` / `get_entry` |
| id 与写入口 | `list_entry_ids` / `create_entry` / `update_entry` / `set_favorite` / `delete_entry` / `delete_entries_tx` |
| 事务封装 | `with_transaction` / `create_entry_tx` / `update_entry_tx` / `set_favorite_tx` / `delete_entry_tx` |
| 快照与已删除 | `export_all_snapshots` / `snapshot` / `list_deleted_entry_ids` / `list_deleted_entries` / `restore_entry_tx` |
| 历史恢复 | `restore_from_history_tx` |
| 导入与清理 | `force_import_entry_tx` / `create_remote_copy_tx` / `purge_deleted` / `upsert_imported_entries` / `vault_crypto_info` |
| 内部写实现 | `query_ids` / `create_entry_in` / `update_entry_in` / `set_favorite_in` / `delete_entry_in` |
| 导入内部实现 | `write_imported_entry_in` / `upsert_imported_in` |
| 快照读取 | `read_snapshot` |
| 字段级加解密 | `encrypt_opt` / `decrypt_opt` / `encrypt_custom_fields` / `decrypt_custom_fields` |

拆分方案（`security_db/crud.rs` → `security_db/entries/`）：

```
security_db/
├── mod.rs              — SecurityDbState + 连接/密钥生命周期（保持现状，不动）
├── entries/
│   ├── mod.rs          — 模块声明 + 子模块公开入口 re-export + query_ids（共享叶子）
│   ├── tx.rs           — with_transaction（事务基础设施，叶子）
│   ├── field_crypto.rs — encrypt_opt / decrypt_opt / encrypt_custom_fields / decrypt_custom_fields（叶子）
│   ├── snapshot.rs     — read_snapshot / snapshot / export_all_snapshots
│   ├── query.rs        — list_entries / get_entry / list_entry_ids / vault_crypto_info
│   ├── write.rs        — create_entry / update_entry / set_favorite / delete_entry / delete_entries_tx
│   │                     + create_entry_in / update_entry_in / set_favorite_in / delete_entry_in
│   │                     + create_entry_tx / update_entry_tx / set_favorite_tx / delete_entry_tx
│   ├── deleted.rs      — list_deleted_entry_ids / list_deleted_entries / restore_entry_tx
│   │                     / restore_from_history_tx / purge_deleted
│   └── import.rs       — write_imported_entry_in / upsert_imported_in / force_import_entry_tx
│                         / create_remote_copy_tx / upsert_imported_entries
├── import_export.rs    — 对外导入导出门面（保持现状）
├── biometric_ops.rs / crypto_ops.rs / meta.rs / rekey.rs / schema.rs（保持现状）
```

依赖方向（单向，无环）：

| 子模块 | 依赖 | 说明 |
|---|---|---|
| `tx` | 无 | `with_transaction` 是叶子，被 write / deleted / import 使用 |
| `field_crypto` | 无 | 纯加解密，无回边 |
| `snapshot` | field_crypto + mod 的 `query_ids` | `read_snapshot` 内含 `decrypt_opt` |
| `query` | 无（仅 `security_db::meta`） | `list_entries` / `get_entry` / `list_entry_ids` 各自内联 SQL，均不调 `read_snapshot` |
| `write` | tx / snapshot / field_crypto | `*_in` 内部 `encrypt_opt`，写后 `read_snapshot` 回读 |
| `deleted` | tx / snapshot / field_crypto | 恢复后回读快照 |
| `import` | tx / snapshot / field_crypto | 导入后回读快照 |

要点：

- **Rust 允许同一 crate 内多个 `impl SecurityDbState` 分布在不同子模块**，因此公开 API 完全不变；inherent impl 方法不需要函数级 re-export，只需在 `mod.rs` 声明 `mod entries;`。
- free fn（`*_in` / `read_snapshot` / `encrypt_*`）一旦被跨子模块调用，就必须 `pub(super)` / `pub(crate)`，必要时由 `entries/mod.rs` re-export——与 6.1 的"对外 API 用 `pub use`"并不冲突，只是适用范围不同。
- **`tx.rs` 只放 `with_transaction`，不放 `*_tx` 封装**。若把 `create_entry_tx` 等与 `with_transaction` 一起放 `tx.rs`，则 `tx` 需调用 `write.rs` 的 `*_in`，而 `write.rs` 的 `delete_entries_tx` 又需 `with_transaction`，形成 `tx ↔ write` 双向环。四个 `*_tx` 与对应 `*_in` 同处 `write.rs`，环即消失。
- **`query_ids` 不能放 `query.rs`**：它的调用方是 `export_all_snapshots`（snapshot.rs）与 `list_deleted_entry_ids`（deleted.rs），而 `list_entry_ids`（query.rs）内联了等价 SQL、并不调用它。若放 `query.rs`，`query ↔ snapshot` 成环；应下沉到 `entries/mod.rs`。
- `read_snapshot` 有 4 条入边（write / import / deleted / snapshot 自身），`snapshot.rs` 是叶子。
- `vault_crypto_info` 依赖 `super::meta::get_salt`（security_db 顶层的 `meta.rs`），这是 entries 之外的一条边，拆分时保留。
- `delete_entries_tx` 带 export 回调，是批量删除入口，归属 `write.rs`；`restore_entry_tx` / `restore_from_history_tx` 虽同带 `_tx`，但属删除恢复语义，归 `deleted.rs`。
- 字段加解密只被 `write.rs` / `import.rs` / `snapshot.rs` 使用，`field_crypto.rs` 处于依赖最底层，无回边。
- 备忘（属"改写"，须与搬移分开提交）：`list_entry_ids` 内联了一份与 `query_ids` 等价的 SQL，可统一为调用 `query_ids`。

难度：低（纯机械搬移，无并发/流状态）。风险：内部实现可见性遗漏导致的编译错误，`cargo check` 会全部暴露。

### 3.2 P0-2 `assistant-vault/sync_engine/mod.rs`

现状（按职责）：

| 职责段 | 内容 |
|---|---|
| imports + 预览/路径 helper | `mask_entry_preview` / `entry_rel_path` / `parse_entry_id` |
| 生命周期与 pending removals | `SyncEngine` / `new` / `pending_removals` / `stage_pending_removals` / `confirm_removals` / `readyness` / `prepare` / `finalize` |
| 拉取 | `pull` / `import_from_repo` |
| 错误归因 helper | `remote_uses_other_master_key` / `attributed_integrity_error` / `unreadable_reason` |
| 推送 | `push` / `sync_now` / `force_push` |
| 冲突 | `conflicts` / `resolve_conflict` / `export_entry_from_db` / `decode_entry_content` / `import_remote_entry` / `create_remote_copy` |
| 配置与远端运维 | `get_config` / `set_config` / `init` / `clone_repo` / `clone_recovery_repo` / `checkout_recovery_repo` / `add_remote` / `set_remote` / `remove_remote` / `ensure_remote` / `remote_url` / `abort_repo_state` / `discard_local_changes` / `test_connection` |
| 历史与预览 | `history_list` / `history_preview` / `conflict_preview` / `reveal_conflict_field` / `history_restore` / `read_historical_entry` / `export_all_entries` |
| 元数据与应急 | `vault_file_manager` / `write_signed_vault_meta` / `write_emergency_data` / `read_emergency_data` / `clear_emergency_data` / `verify_vault_meta_integrity` / `rewrite_vault_meta_with_new_key` / `disaster_recovery_export` |
| 自由函数 | `collect_conflicts` / `attach_conflict_titles` / `diff_entry_fields` / `to_crypto_meta` |

拆分方案（`sync_engine/mod.rs` → `sync_engine/`）：

```
sync_engine/
├── mod.rs          — SyncEngine 结构 + new + vault_file_manager 访问器 + 子模块声明（叶子根）
├── preview.rs      — mask_entry_preview / entry_rel_path / parse_entry_id / collect_conflicts
│                     / attach_conflict_titles / diff_entry_fields（纯函数叶子，不依赖 SyncEngine）
├── lifecycle.rs    — readyness / prepare / finalize（纯 git_sync 委托叶子）
├── export.rs       — export_all_entries / write_signed_vault_meta / verify_vault_meta_integrity
│                     / to_crypto_meta（DB → vault 文件写盘与 vault_meta 边界）
├── pull_push.rs    — pending_removals / stage_pending_removals / confirm_removals
│                     / pull / import_from_repo / 错误归因 helper / push / sync_now / force_push
├── remote_admin.rs — get_config / set_config / init / clone_repo / clone_recovery_repo
│                     / checkout_recovery_repo / add_remote / set_remote / remove_remote / ensure_remote
│                     / remote_url / abort_repo_state / discard_local_changes / test_connection
├── history.rs      — conflicts / resolve_conflict / export_entry_from_db / decode_entry_content
│                     / import_remote_entry / create_remote_copy / history_list / history_preview
│                     / history_restore / read_historical_entry / conflict_preview / reveal_conflict_field
└── emergency.rs    — write_emergency_data / read_emergency_data / clear_emergency_data
                      / rewrite_vault_meta_with_new_key / disaster_recovery_export
```

依赖方向（单向，无环）：

| 子模块 | 依赖 | 说明 |
|---|---|---|
| `mod` | 无 | `SyncEngine` 结构与 `vault_file_manager` 访问器，被全部子模块使用 |
| `preview` | 无 | 纯函数，不依赖 `SyncEngine` |
| `lifecycle` | mod | 仅委托 `self.git_sync`，被 pull_push / history 共用 |
| `export` | mod | 经 `self.vault_file` 写盘 |
| `pull_push` | preview / lifecycle / export | `pull` / `sync_now` 调 `collect_conflicts` / `attach_conflict_titles` / `prepare` / `finalize` / `export_all_entries` |
| `remote_admin` | export | `init` 调 `write_signed_vault_meta` |
| `history` | preview / lifecycle | `conflict_preview` 调 `mask_entry_preview` / `diff_entry_fields`；`history_restore` 调 `readyness` / `finalize`；`resolve_conflict` 调 `finalize` |
| `emergency` | mod | 应急数据读写与灾难恢复导出 |

要点：

- `SyncEngine` 字段保持私有即可：子模块是 `mod.rs` 的后代，Rust 的可见性规则允许后代模块直接访问祖先模块的私有字段，无需 `pub(super)` 字段或额外 getter。
- **pending removals 三件套归 `pull_push.rs`，不能留 `mod.rs`**：`stage_pending_removals` 被 `pull` / `sync_now` 调用，而 `confirm_removals` 自身要调 `prepare` / `export_all_entries` / `finalize`。若留在 `mod.rs`，`pull` / `push` / `sync_now` 调 `self.pending_removals()` 形成 `mod → pull_push`，`confirm_removals` 形成 `mod → export` 与 `mod → lifecycle`，与 `pull_push → mod` 构成双向环。
- **`readyness` / `prepare` / `finalize` 独立为叶子 `lifecycle.rs`**：它们同时被 `pull_push`（`sync_now` / `confirm_removals`）与 `history`（`history_restore` / `resolve_conflict`）调用。挂在任一编排子模块都会让另一侧产生"历史依赖编排"的跨层边；独立成叶子后两侧只依赖它，耦合消失。
- **`export_all_entries` 归 `export.rs`，不归 `emergency.rs`**：它的调用方是 `pull` 与 `sync_now`，若放 emergency 会产生"编排依赖应急"的反向语义边；它本质是"DB 快照 → 写 vault 条目文件 → 删已删条目文件 → 写 vault_meta"，是 pull/push 的前置写盘步骤。
- `to_crypto_meta` 只被 `export_all_entries` 与 `write_signed_vault_meta` 使用，两者都在 `export.rs`，因此随之下沉，不能放 `preview.rs`。
- `SyncEngine::verify_vault_meta_integrity` 是纯 passthrough，crate 内零调用——`import_from_repo` 调的是 `self.vault_file.verify_vault_meta_integrity`，属 `VaultFileManager` 而非本方法。仍按 vault_meta 边界归 `export.rs`。
- `vault_file_manager` 是访问器，留在 `mod.rs`。
- 拆分后最大子模块为 `history.rs`（327）与 `pull_push.rs`（292）；若日后继续增长，按职责再分，不按行数切。

难度：中（`Mutex` 字段访问与借用需调整，但无并发语义变化）。

### 3.3 P0-3 `assistant-security/unlock_dialog/windows/mod.rs` ✅

拆分前现状（954 行，按职责）：

| 职责段 | 内容 |
|---|---|
| imports / uses | 模块引入 |
| 状态 | `DialogState` / `DialogStatePtr` |
| FFI 与常量 | `extern "system"` 声明、Win32 常量、`make_hmenu` / `get_x_lparam` / `get_y_lparam` |
| 入口 | `Win32UnlockDialog` + `unsafe impl Send/Sync` + `new` + `UnlockDialog::show`（注册窗口类、启动线程、消息循环） |
| 输入框子类化 | `edit_subclass_proc` / `call_original_edit_proc` |
| 按钮子类化与图标绘制 | `btn_subclass_proc` / `get_parent_dialog` / `get_icon_rect` / `is_in_icon_area` / `is_cursor_in_icon_area` / `draw_visibility_icon` |
| 窗口过程 | `wnd_proc`（`WM_NCCREATE` / `WM_CREATE` / `WM_ERASEBKGND` / `WM_CTLCOLOR*` / `WM_DRAWITEM` / `WM_COMMAND` / `DM_GETDEFID` / 键盘 / `WM_CLOSE` / `WM_DESTROY`） |
| 命令辅助 | `take_cancel_callback` / `toggle_password_visibility` |
| 明文读写 | `read_edit_text` / `read_edit_text_secure` / `clear_edit_text` |
| 深色按钮自绘 | `draw_dark_button` |

拆分方案（`windows/mod.rs` → `windows/`，与既有 `layout.rs` / `theme.rs` / `localization.rs` 同级）——**已落地**：

```
unlock_dialog/windows/
├── mod.rs        — 模块声明 + Win32UnlockDialog（new / Send+Sync）+ UnlockDialog::show + 消息循环（140 行）
├── ffi.rs        — extern "system" 声明、Win32 常量、make_hmenu / lparam 解包（46 行）
├── state.rs      — DialogState / DialogStatePtr + 6 个 static mut 全局单例：
│                   ORIGINAL_EDIT_PROC / ORIGINAL_BTN_PROC / BTN_HOVER_UNLOCK
│                   / BTN_HOVER_CLOSE / ICON_FONT / DEFAULT_FONT（全部 pub(super)）（27 行）
├── subclass.rs   — edit_subclass_proc / call_original_edit_proc / btn_subclass_proc
│                   / get_parent_dialog / get_icon_rect / is_in_icon_area
│                   / is_cursor_in_icon_area / draw_visibility_icon
│                   / toggle_password_visibility（303 行）
├── wnd_proc.rs   — 窗口过程骨架 + 按消息类别委托（34 行）
├── wm_handlers.rs— on_nc_create / on_create / on_command / on_keydown / on_close
│                   / on_destroy / take_cancel_callback（352 行）
├── paint.rs      — on_erase_bkgnd / on_ctl_color / on_draw_item / draw_dark_button（109 行）
├── text.rs       — read_edit_text / read_edit_text_secure / clear_edit_text（64 行）
├── layout.rs / theme.rs / localization.rs（保持现状）
```

落地结果：`mod.rs` 954 → 162 行（非空 140），职责数 7 → 1；`cargo check --all-targets`、`cargo clippy -p assistant-security -- -D warnings`、`cargo test -p assistant-security --all-targets` 全部通过（15 项测试）。

要点：

- `extern "system"` 回调必须是单函数（无法拆），但函数体内可立即委托给普通函数：`wnd_proc` 只保留 `match msg` 与 `unsafe` 边界，业务放 `wm_handlers.rs`。
- **6 个 `static mut` 必须集中放 `state.rs`**，这是本文件拆分最关键的一条：`ORIGINAL_EDIT_PROC` / `ORIGINAL_BTN_PROC` 由 `WM_CREATE`（wnd_proc）写入、由 `subclass.rs` 读并调用；`ICON_FONT` 由 `WM_CREATE` 写入、由 `subclass.rs` 的图标绘制读取；`BTN_HOVER_*` 由 `subclass.rs` 写入、由 `paint.rs` 读取。若按函数就近定义，会产生 `wnd_proc ↔ subclass` 双向 `use super::`，且 `static mut` 跨模块访问必须 `pub(super)` 并置于 `unsafe` 块，是 `unused_unsafe` 与别名冲突的高发点。
- 依赖方向：`wnd_proc` / `wm_handlers` / `subclass` / `paint` / `text` → `state` + `ffi`（单向）；`wm_handlers` → `subclass`（注册子类化过程）与 `paint`（`WM_PAINT` 委托）单向。`subclass` 与 `paint` 之间只通过 `state.rs` 的 `BTN_HOVER_*` 交换状态，不直接引用对方。
- `toggle_password_visibility` 实际留在 `subclass.rs`：它需读写 `EM_SETPASSWORDCHAR` 并操作输入框子类化状态，放 `wm_handlers` 会引入 `wm_handlers → subclass` 的额外耦合，故与原 `subclass` 段同置。
- 该文件**无自动化测试**，验证依赖手工解锁流程（输入/粘贴/回车/Esc/可见性切换/DPI 缩放/深色主题）。拆分后必须完整手测一次，且只搬移不改写。
- 仅 Windows 目标编译，`cfg(target_os = "windows")` 边界已由 `unlock_dialog/mod.rs` 控制，拆分不改变跨平台门禁。
- 分两步落地：先搬 `ffi` / `state` / `text` / `subclass` / `paint`（纯搬移），再拆 `wnd_proc`（骨架 + 委托，涉及 `GWLP_USERDATA` 解包与 `unsafe` 边界）。

难度：高（大量 `unsafe` 与裸指针，状态经 `GWLP_USERDATA` 传递）。风险：跨模块搬移时 `DialogStatePtr` 与 `Send/Sync` 的可见性需 `pub(super)`。

### 3.4 P1-1 `assistant-providers/openai/responses_stream.rs`

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
- `StreamState` 需要新定义为独立结构（承载 `tool_calls_by_output_index`、`done_items`、`unified_responses`），这是本次拆分的核心重构点。
- 先做小步验证：只抽 `response.completed` 分支，确认 `cargo check` 通过后再推广。
- 这是真重构而非机械搬移：需新引入 `StreamState` 状态结构，事件分发顺序、delta 生成时机的语义极易漂移。建议先补 `handle_event` 级 fixture 再动手，拆分后可复用。

难度：高。

### 3.5 P1-2 `assistant-agent/multimodal.rs`

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

### 3.6 P2 归位清单（300–499 行，按职责数决定动作）

| 文件（参考非空行） | 动作 | 目标结构 |
|---|---|---|
| `assistant-sync/git_sync/git.rs`（491） | 拆 | `git/{mod,repo,branch,remote,log,plumbing}.rs` |
| `src-tauri/src/cmd/conversation.rs`（467） | 拆 | `cmd/conversation/{mod,list,detail,detail_dto,mutate,branch,export}.rs`；DTO 组装移入 `detail_dto.rs` |
| `assistant-tools/os/fs_link.rs`（453） | 拆 | `os/fs_link/{mod,validate,create,check}.rs`（平台分支集中在 create/check） |
| `assistant-providers/gemini/request.rs`（448） | 拆 | `gemini/request/{mod,body,stream,url}.rs` |
| `assistant-security/screen_lock/mod.rs`（435） | 拆 | `screen_lock/{mod,windows,macos,linux}.rs`，与 `unlock_dialog` 范式对齐 |
| `assistant-providers/gemini/message_converter.rs`（431） | 拆 | `message_converter/{messages,tools,schema}.rs` |
| `src-tauri/src/protocol/dto/chat.rs`（422） | 拆 | 按域：`dto/chat/{conversation,message,usage,event,content}.rs` |
| `assistant-providers/anthropic/request.rs`（400） | 拆 | 与 gemini 同构：`anthropic/request/{mod,body,stream}.rs` |
| `assistant-vault/dto/convert.rs`（400） | 拆 | `dto/convert/{sync,entry,totp,error,hibp}.rs`；`sync` 一类占 15 个 impl，需再按 `conflict / history / config / pending` 细分（该文件无 `vault_meta` 领域） |
| `assistant-providers/tool_call_accumulator.rs`（398） | 仅移测试 | 实现留在原文件，测试移 `tests/tool_call_accumulator.rs`（沿用 `internal-tests` feature） |
| `assistant-providers/client/quirks.rs`（377） | 仅移测试 | 实现留在原文件，测试外移 |
| `src-tauri/src/protocol/dto/mod.rs`（331） | 仅移测试 | round-trip 测试移 `src-tauri/tests/dto_roundtrip.rs`，实现保持 re-export |
| `assistant-vault/vault_file/mod.rs`（389） | 拆 | `vault_file/{mod,entry,meta,emergency}.rs` |
| `assistant-agent/chat_runtime.rs`（375） | 拆 | `chat_runtime/{types,accumulator,consume}.rs` |
| `src-tauri/src/cmd/vault/sync.rs`（371） | 拆 | `cmd/vault/sync/{mod,git,history,conflict}.rs` |
| `src-tauri/src/cmd/compaction.rs`（367） | 拆 | `cmd/compaction/{mod,pipeline,persist}.rs`；`execute_compaction` 按"构建提示 → 调 provider → 消费流 → 落库"四段拆 |

> 移测试的 3 个文件（`tool_call_accumulator` / `quirks` / `protocol/dto/mod.rs`）不属于职责混杂，不应拆实现；它们的长度问题由内联测试造成。

### 3.7 P3 观察名单（职责单一，暂不动）

`gemini/stream.rs`、`anthropic/message_converter.rs`、`cmd/vault/entries.rs`、`openai/chat.rs`、`gemini/code_assist.rs`、`openai/stream.rs`、`stream/inline_think.rs`、`types/chat_delta.rs`、`openai/responses_types.rs`、`src/window.rs`、`security_db/mod.rs`。

触发条件：任一文件出现第二个变化原因（职责数增至 ≥3）时纳入下一轮；不因行数增长自动触发。

---

## 4. 已完成（不得回退）

| 项 | 结果 | 现状校验 |
|---|---|---|
| Phase 1：`storage/conversations.rs` → 目录模块 | mod / crud / msg_ops / tree_ops / ops / compression / branch | storage 最长文件为 `agent_modes.rs` |
| I21：`send_chat_message` 上帝函数 → Pipeline | `chat.rs` + `chat_insert.rs` + `chat_request.rs`，主函数收窄为编排 | `cmd/chat/` 现为 mod / insert / stream |
| I22：`chat/request.rs` + `chat/utils.rs` 下放 `assistant-agent` | `chat_request.rs` + `chat_runtime.rs` 追加 tokens 解析；`build_provider_config` 收敛 3 调用点；models_json 解析收敛 `assistant-providers::types` 单一入口 | `cmd/conversation_utils.rs` 仅剩薄封装（重复逻辑已删） |
| Phase A：`security_db/crud.rs` → `security_db/entries/` | tx / field_crypto / snapshot / query / write / deleted / import 七子模块 + `entries/mod.rs`（`query_ids` 共享叶子）；free fn 可见性收窄为私有或 `pub(super)` | `crud.rs`（1198）已删除；`security_db/` 下最长文件为 `entries/write.rs`（271）与 `entries/import.rs`（268） |
| Phase B：`sync_engine/mod.rs` → 8 文件 | mod（结构根）/ preview / lifecycle / export / pull_push / remote_admin / history / emergency；pending removals 三件套下沉 `pull_push`，`readyness` / `prepare` / `finalize` 独立为 `lifecycle` 叶子 | `sync_engine/mod.rs` 由 1149 行降至 33 行；最长子模块 `history.rs`（327） |
| Phase C：`unlock_dialog/windows/mod.rs` → 8 文件 | mod（入口 + 消息循环）/ ffi / state（6 个 `static mut` 集中）/ text / subclass / paint / wm_handlers / wnd_proc；`toggle_password_visibility` 随子类化状态留 `subclass.rs`；公开 API（`Win32UnlockDialog` / `UnlockDialog` impl）零变更 | `windows/mod.rs` 由 954 行降至 162 行（非空 140）；最长子模块 `wm_handlers.rs`（352） |

上述文件在后续职责数增长时，按本方案重新入观察。

---

## 5. 执行计划

| Phase | 目标 | 顺序理由 | 难度 |
|---|---|---|---|
| A ✅ | `security_db/crud.rs` → `entries/` | 纯机械搬移，无并发与流状态；先建立"拆一个文件一个 PR"的节奏 | 低 |
| B ✅ | `sync_engine/mod.rs` → 8 文件 | 与 A 同 crate，可复用 A 验证过的子模块范式；主要风险是 `SyncEngine` 私有字段跨模块访问，单独 PR 便于定位 | 中 |
| C ✅ | `unlock_dialog/windows/mod.rs` → 8 文件 | 全文件为 `unsafe` 原生窗口代码，需预留手测时间；先纯搬移、再拆 `wnd_proc` | 高 |
| D | `responses_stream.rs` + `multimodal.rs` | 两者都需引入状态结构，属真实重构，需独立 PR + 回归验证 | 高 |
| E | P2 清单按"仅移测试 → 平台拆分 → 领域拆分"次序推进 | 低风险项先行，控制单 PR 体积 | 中 |

每个 Phase 的完成判据（在 `src-tauri` 目录执行）：

```
cargo check
cargo test
cargo clippy --all-targets -- -D warnings
```

注意：`assistant-vault` 的 `tests/sync_engine.rs` 挂在 `#![cfg(feature = "git-tests")]`、`tests/vault_crud.rs` 挂在 `#![cfg(feature = "slow-tests")]`，两个门禁都需显式开启，否则对应测试会显示 `running 0 tests`：

```
cargo test -p assistant-vault --features slow-tests
cargo test -p assistant-vault --features git-tests --test sync_engine
```

Phase C 无自动化测试覆盖，除上述三条命令外，还需手工走一遍解锁流程（输入 / 粘贴 / 回车 / Esc / 可见性切换 / DPI 缩放 / 深色主题 / 按钮 hover 与焦点框）。

外加：对外可见的函数签名与类型定义零变更；子模块间无循环依赖；每个子模块只对应一个变化原因。**不设行数目标**——子模块大小由职责决定，若某个子模块仍承担两类变化原因，继续按职责拆，而不是按行数切。

---

## 6. 拆分规范

### 6.1 目录模块模式

1. `foo.rs` → `foo/mod.rs` + 子模块；`mod.rs` 只放类型定义、共享常量、组合入口、模块声明。
2. 子模块按**职责**命名（`query` / `write` / `history` / `emergency`），不按行号或 `part1` / `part2`。
3. 共享项用 `pub(super)` 暴露；对外 API 用 `pub use sub::*` 保持不变（inherent impl 方法无需 re-export，free fn 需要）。
4. 同一 crate 内可存在多个 `impl` 块（如 `impl SecurityDbState` 分散在 `entries/*.rs`），这是首选的低风险手段，不必强行收敛到单文件。

### 6.2 职责边界判据

- 一个文件对应一个"变化原因"：同步策略、加密算法、UI 布局、DTO 契约互不混居。
- 组合入口（如 `get_conversation_detail`、`sync_now`）留在上层 `mod.rs`，避免子模块互相调用形成环。
- 平台相关代码按平台分文件，用 `cfg` 在父模块收口。

### 6.3 禁止事项

- 不为拆分而拆分：职责单一的文件不动，与行数无关（3.6 P2 清单中的职责混杂文件除外）。
- 不改公开 API：函数签名、类型定义、`pub use` 路径全部保持。
- 不引入循环依赖：子模块只通过 `super::` 单向引用。
- 不在 `async_stream::stream!` 宏外使用 `yield`。
- 不把测试当作拆分理由：内联测试超标时移测试，不拆实现。
- 不混合提交：一个 PR 只拆一个文件；"搬移"与"改写"分开提交，便于 review 与回滚。

---

## 7. 预期收益

按职责而非行数描述：

- 职责数 ≥5 的文件：3 → 0。
- 上千行的单体文件：2 → 0。
- 最大单文件显著下降，衡量标准是"每个子模块只对应一个变化原因"，不设行数目标。
- 平台相关代码（`screen_lock`、`fs_link`）与原生 UI（`unlock_dialog/windows`）按平台/职责分文件，与既有 `linux.rs` / `macos.rs` / `windows/` 范式一致。
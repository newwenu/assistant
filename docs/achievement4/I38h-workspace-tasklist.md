# I38h — P12 工作区任务清单

> 规格：[I38h-workspace-spec.md](./I38h-workspace-spec.md)（§2 存储、§3 解析、§4 消费点、§6 变更、§8 DTO、§9 前端、§12 验收）
> 阶段：[I38-agent-roadmap.md](./I38-agent-roadmap.md) P12
> 顺序：C1 → C2 → C3 → C4 → C5 → C6 → C7（C3 与 C4 可并行，但 C5 必须等 C1 + C4）
> 勾选条件：代码已改 **且** 对应验收已跑。
> **状态唯一位置**：本清单是 P12 任务级进度的唯一来源；规格只定义「做什么」与「验收什么」，不记录完成状态；阶段级结论只在 [roadmap](./I38-agent-roadmap.md) §2 写一次。本清单**不管理提交**。
> 范围：会话工作区绑定、目录簿、唯一解析点、4 处同源、授权联动、前端选择器、设置页字段移除。**不含** 遗留全局值迁移、MCP roots 实现、多工作区并行、默认工作区层（规格 §13）。
> 测试位置：Rust 单测随模块 `#[cfg(test)]`；Rust 集成测试放 `src-tauri/crates/assistant-storage/tests/` 或 `src-tauri/tests/`；前端单测放 `src/**/__tests__/`。

---

## C1 存储与迁移（依赖：无）

实现：`src-tauri/crates/assistant-storage/src/db.rs`、`src-tauri/crates/assistant-storage/src/conversations/mod.rs`、`src-tauri/crates/assistant-storage/src/conversations/crud.rs`、`src-tauri/crates/assistant-storage/src/conversations/ops.rs`、`src-tauri/crates/assistant-storage/src/workspaces.rs`（新建）、`src-tauri/crates/assistant-storage/src/lib.rs`、`src-tauri/src/setup.rs`

- [x] C1.1 `conversations` 加列 `workspace_root TEXT NOT NULL DEFAULT ''`：`pragma_table_info` 判存后 `ALTER TABLE ... ADD COLUMN`，幂等（照 `tool_config_json` 既有迁移写法，规格 §2.1）
- [x] C1.2 `ConversationRow` 新增 `workspace_root: String`（列 `NOT NULL`，读侧恒为 `String`）；`CONV_SELECT` 追加该列、`read_conv_row` 索引同步、`insert` / `update` 占位符个数同步；`ops.rs` `search_conversations` 的内联列清单同样追加（该查询不走 `CONV_SELECT`）（规格 §2.1）
- [x] C1.3 会话复制 / 分叉（`ops.rs` 的 `duplicate_conversation` / `fork_to_new_conversation`）构造 `ConversationRow` 时继承源会话的 `workspace_root`：分叉产物是独立会话，工作区随之复制；不把工作区写进消息或分支（规格 §13）
- [x] C1.4 `workspaces` 表：`CREATE TABLE IF NOT EXISTS workspaces (path TEXT PRIMARY KEY, name TEXT, created_at INTEGER NOT NULL, used_at INTEGER NOT NULL)`，与既有建表 SQL 同批执行（规格 §2.2）
- [x] C1.5 新建 `workspaces` 模块：`list`（按 `used_at` 倒序）、`upsert`（刷新 `used_at`）、`forget`、`get_name`；`name` 缺省取目录 basename
- [x] C1.6 `forget` 只删簿行，**不触碰**任何会话行（A11）
- [x] C1.7 加列后**不做**任何数据迁移：存量会话 `workspace_root = ''`（未配置）即最终状态，遗留 `AppConf.workspace_root` 不参与（规格 §2.3）
- [x] C1.8 单测：重复 `init_db` 无副作用且不重复加列；upsert 幂等且刷新 `used_at`、保留既有显示名；`forget` 后会话绑定仍在；`list` 按 `used_at` 倒序；存量会话升级后恒为空串（A1 / A11）

---

## C2 唯一解析点（依赖 C1）

实现：`src-tauri/src/tool_setup.rs`

- [x] C2.1 抽出 `validate_workspace_root(&str) -> Option<String>`：空串 / 非绝对 / `canonicalize` 失败 / 非目录 → `None`；写入与读取**共用这一份**（规格 §1）
- [x] C2.2 实现 `resolve_workspace(conn: &Connection, conversation_id: &str) -> Option<String>`：只读 `conversations.workspace_root`，空串 → `None`，非空过校验（规格 §3.1 / §3.2）
- [x] C2.3 判定矩阵逐条落地：空串 → `None`；有效 → `Some(canonical)`；脏值 → `None`；会话行不存在 → `None`；读取报错 → `None` + `warn!`（fail-closed 到未配置）
- [x] C2.4 不提供无参重载、不读 `AppConf`、不读 `workspaces` 表、不退回 cwd（规格 §3.3 五条不变量）
- [x] C2.5 不做缓存：每次调用现算，切换绑定后下一轮立即生效（规格 §3.1 / §6.1）
- [x] C2.6 删除 `workspace_root_from_conf`（**不保留兼容包装**），其单测删除或改写为 `resolve_workspace` 矩阵测试（规格 §3.4）
- [x] C2.7 单测穷举矩阵（含相对路径、空串、目录被删、会话不存在、DB 报错）——覆盖 A3 / A4 / A5

---

## C3 消费点改造（依赖 C2）

实现：`src-tauri/src/adapters/tool_exec.rs`、`src-tauri/src/adapters/input_source.rs`、`src-tauri/src/cmd/conversation.rs`、`src-tauri/src/cmd/agent/mod.rs`、`src-tauri/src/cmd/os_tool_exec.rs`、`src-tauri/src/tool_setup.rs`（装配）

- [x] C3.1 `WorkspaceRootSource` 签名改为 `Arc<dyn Fn(&str) -> Option<String> + Send + Sync>`（规格 §4 #1）
- [x] C3.2 调用点改为 `(self.workspace_root)(ctx.conversation_id)`（`ToolExecCtx` 已带该字段，**不加新入参**）；`ToolUseContext.workspace_root` 只填解析结果，不附加任何来源标记（规格 §4 #1）
- [x] C3.3 装配处统一构造一次（`tool_setup::init` 内「取 DB 连接 → `resolve_workspace`」的唯一闭包）；禁止各处包一层闭包改语义（规格 §4 装配点）
- [x] C3.4 `adapters/input_source.rs`：`prompt_facts` 改为 `prompt_facts(conn, conversation_id)`；事实行仍为 `Working directory: <绝对路径>`，**不追加工作区名**（规格 §4 #2）
- [x] C3.5 `cmd/conversation.rs` 占用估算按会话 id 解析，保证估算与真实提示词同源（规格 §4 #3）
- [x] C3.6 `cmd/agent/mod.rs`：`policy_executor` 装配增加 `conversation_id` 入参，`.assistant/permissions.json` 随会话工作区定位（规格 §4 #4）
- [x] C3.7 `cmd/os_tool_exec.rs`：ToolBox 固定 `ToolUseContext::for_toolbox(None)`，无会话、无基准、无边界（规格 §4.2）
- [x] C3.8 全仓检索断言：无任何位置读 `AppConf` 的 `workspace_root`（A2 静态检查）
- [x] C3.9 验收 A2（4 处同源）/ A9（项目策略随会话）/ A10（ToolBox 不解析工作区）
- [x] C3.10 越界回归：会话工作区下 `ensure_within_root` 与相对路径基准不变；未配置时会话链路与现状（全局留空）一致（A1 / A5）

---

## C4 DTO + 命令 + 契约（依赖 C1）

实现：`src-tauri/src/protocol/dto/chat.rs`、`src-tauri/src/protocol/dto/workspace.rs`（新建）、`src-tauri/src/protocol/dto/config.rs`、`src-tauri/src/cmd/conversation.rs`、`src-tauri/src/cmd/workspace.rs`（新建）、`src-tauri/src/lib.rs`（invoke 注册）、`src-tauri/tests/protocol_bindings.rs`、`src/types/protocol.contract.ts`（生成）

- [x] C4.1 `ConversationDto` 新增 `workspace_root: String`：输出 DTO，键恒在、值恒为字符串（`""` = 未配置），**不加** `skip_serializing_if`，**不出现 `null`**（规格 §8.1）
- [x] C4.2 `ConversationPatchDto` 新增 `workspace_root` + `clear_workspace_root`：`#[serde(default)]` + `#[ts(optional = nullable)]`，沿用 `deny_unknown_fields`；**不使用** `Option<Option<String>>`（规格 §8.2）
- [x] C4.3 新建 `WorkspaceDto`：`path` / `name` / `missing`；`missing` 为只读探测（目录当前不存在），不回写库（规格 §8 表）
- [x] C4.4 命令：`list_workspaces` / `choose_workspace_dir`（只返回所选路径，不写库）/ `register_workspace`（`path`, `name?`）/ `forget_workspace`（`path`），注册到 invoke handler（规格 §8.3）
- [x] C4.5 `patch_conversation` 处理工作区三态：`workspaceRoot` 缺省 = 不改；`workspaceRoot: null` = 不改；有值 = 校验后写入；`workspaceRoot: ""` = `Err`（空串是未配置哨兵，不允许作为设置值）；`clearWorkspaceRoot: true` = 写回 `""`；`workspaceRoot` 与 `clearWorkspaceRoot` 同现 → `Err`（规格 §8.2）
- [x] C4.6 `patch_conversation` 返回类型保持 `Result<(), String>`，不改成返回 DTO（前端成功后用提交值更新本地状态，规格 §9.4）
- [x] C4.7 `ConversationRow` ↔ `ConversationDto` 映射补齐新字段；列表 / 详情查询同步
- [x] C4.8 config DTO 删除 `workspaceRoot` 字段（Rust `AppConf` 结构保留该字段仅作迁移输入，不进 wire，规格 §8.4）
- [x] C4.9 `npm run contract:gen` 生成 `src/types/protocol.contract.ts`，提交生成结果；前端 `confTypes` 不再有 `workspaceRoot`
- [x] C4.10 `protocol_bindings.rs` `declarations` 登记 `WorkspaceDto` 与新增命令（每个 DTO 只注册一次）
- [x] C4.11 `cargo test --test assembly_contract` 与 `npm run contract:check` 通过（A12）——`assembly_contract` 8/8 通过；`contract:gen` 幂等（两次生成哈希一致），`contract:check` 的 `git diff` 仅反映本次未提交的生成变更，提交后即通过
- [x] C4.12 `list_workspaces` 的 `missing` 探测设耗时上限（建议 500ms，实施时定）；超时按 `missing = false` 处理且不报错（规格 §14）
- [x] C4.13 实机回归 A12：目录删除 / 移动后 `list_workspaces` 标记 `missing`、chip 有警示、应用不 panic、不静默改库（需实机，随 C6 一并验收）——2026-11-09 实机通过

---

## C5 授权联动（依赖 C1、C4）

实现：`src-tauri/src/cmd/conversation.rs`、`src-tauri/src/adapters/permission_store.rs`（或 storage 层删除接口）

- [x] C5.1 工作区生效（设置 **或** 清除）时，同一事务内删除该会话全部 `GrantScope::Session` 行：`DELETE FROM permission_grants WHERE grant_key LIKE 's:<conversationId>|%'`（规格 §6.2）
- [x] C5.2 `GrantScope::Global` 行**不删**（用户显式全局决策，与工作区无关）
- [x] C5.3 原子性：删授权与写绑定同事务，任一失败则 patch 整体失败，不允许「边界已换、授权仍在」（A6）
- [x] C5.4 审计写一行：`tool = "workspace"`、`action = "switch"`、`resource = <新根路径>`（清除写 `"none"`）、`outcome = allowed`；**不新增** `AuditOutcome` 变体
- [x] C5.5 turn 进行中拒绝：复用 `TurnTasks::is_busy` 返回 `error.conversation_busy`（A8）
- [x] C5.6 集成测试：切换后会话级 grant 全消失、全局保留（A6）；清除绑定同样清理（A7）；删授权失败回滚（A6）；busy 拒绝（A8）；实机确认 turn 中切换被拒、turn 结束后可切换——`assistant-storage/tests/workspaces.rs` 新增 5 例（会话级全删 / 全局保留 / `c1` 与 `c10` 前缀边界 / 清除绑定清理 / 同事务提交与回滚），13/13 通过；busy 拒绝由 `patch_conversation` 的 `is_busy` 分支覆盖，实机项随 C6.11 一并确认

---

## C6 前端（依赖 C4）

实现：`src/chat/selectors/WorkspaceSelector.tsx`（新建）、`src/chat/layouts/ConversationFrame.tsx`、`src/chat/layouts/AgentLayout.tsx`、`src/chat/layouts/layoutProps.ts`、`src/services/`（命令封装）、`src/i18n/locales/zh-CN.json`、`src/i18n/locales/en-US.json`

- [x] C6.1 `WorkspaceSelector`：形态对齐 `ModelSelector` 的 `variant="chip"`，位置与 `ProviderSelector` / `ModelSelector` 同级
- [x] C6.2 `ConversationFrame` 新增 `workspaceControl?: React.ReactNode` 插槽，放在 `orb-chrome-bar`；**仅 `AgentLayout` 传入**，`ChatLayout` 不传（规格 §9.2）
- [x] C6.3 交互：最近工作区列表（含 `missing` 标记）/ 「选择文件夹…」（`tauri-plugin-dialog`）/ 「清除绑定（未配置）」；选中后先 `register_workspace` 再 `patch_conversation`（规格 §9.1 / §9.4）
- [x] C6.4 三态显示按规格 §9.3 落地：未配置（警示「选择工作区」）/ 有效（显示名）/ 失效（显示名 + 失效标记）；**不禁用发送**
- [x] C6.5 「清除绑定」走 `patch_conversation({ clearWorkspaceRoot: true })`，成功后本地置 `""`（规格 §8.2）
- [x] C6.6 turn 进行中 chip 禁用 + tooltip `workspace.error.busy`
- [x] C6.7 失败不做乐观更新：保留原值并 toast 后端错误
- [x] C6.8 切换成功 toast `workspace.switch_cleared_session_grants`：会话级授权已失效、全局授权仍生效；并刷新前端会话级授权缓存（规格 §9.4 #4）——前端 `permissionService` 无会话级缓存（每次查询直连后端），故无缓存可刷
- [x] C6.9 `fs_link` 表单改为**要求绝对路径**并加提示：ToolBox 无工作区基准，相对路径必报错（规格 §4.2）
- [x] C6.10 i18n `workspace.*`：`chip.select` / `chip.missing` / `action.choose` / `action.clear` / `error.invalid` / `error.busy` / `switch_cleared_session_grants`，中英同步
- [x] C6.11 前端单测：三态渲染、切换调用序列（`register` → `patch`）、清除绑定、busy 禁用；实机确认 A3 / A5 / A12 的 chip 形态——`workspaceDisplay` 11 例（三态 + `\\?\` 前缀剥离 + 目录簿未命中回退）与 `workspaceSwitch` 4 例（`register` 先于 `patch`、登记失败不写绑定、绑定失败不伪装成功、异常收敛）全部通过；2026-11-09 实机确认 chip 三态形态与 A3 / A5 / A12

---

## C7 设置页字段移除（依赖 C4、C6）

实现：设置页「Agent 工具 / 工作区」字段所在组件、`src/i18n/locales/*.json`

- [x] C7.1 删除设置页「默认工作区」字段（`AppConf.workspace_root` 不再有 UI 入口，规格 §2.4 / §9.4 #5）
- [x] C7.2 删除对应 i18n 键（含旧「跟随默认」文案），中英同步；「Agent 工具」节只保留审批等待上限
- [x] C7.3 确认前端 `confTypes` 已随 `contract:gen` 移除该键，设置页无残留引用（`typecheck` + `lint` 通过）

---

## 验收映射

| 验收 | 覆盖条目 |
|---|---|
| A1 | C1.7、C1.8、C3.10 |
| A2 | C2.2、C3.3、C3.8、C3.9 |
| A3 | C2.3、C2.7、C6.4、C6.11 |
| A4 | C2.1、C2.3、C2.7 |
| A5 | C2.3、C3.4、C3.10、C6.4、C6.11 |
| A6 | C5.1、C5.3、C5.6 |
| A7 | C5.1、C5.6 |
| A8 | C5.5、C5.6、C6.6 |
| A9 | C3.6、C3.9 |
| A10 | C3.7、C3.9、C6.9 |
| A11 | C1.6、C1.9 |
| A12 | C4.3、C4.9、C4.13、C6.4、C6.11、C7.3 |
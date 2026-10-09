# I6 — Agent 自定义模式 + Prompt 模板合并 任务清单

> 规格：[I6-spec.md](./I6-spec.md)
> 前置：I2 ✅ + I4 ✅（chatStore 流式输出 + 设置系统已就绪）

---

## 阶段 A — Rust 后端：数据层 + CRUD

- [x] A1. 新增 `assistant-storage/src/agent_modes.rs`：`AgentModeRow` 结构体定义（id / name / description / kind / prompt / model / temperature / max_tokens / tools_json / presentation_rank / is_builtin / created_at / updated_at）
- [x] A2. `agent_modes.rs` `init_table(conn)` — CREATE TABLE IF NOT EXISTS agent_modes
- [x] A3. `agent_modes.rs` `insert_mode(conn, row)` — INSERT OR IGNORE
- [x] A4. `agent_modes.rs` `get_modes(conn)` — SELECT 全部，按 presentation_rank 排序
- [x] A5. `agent_modes.rs` `get_by_id(conn, id)` — SELECT WHERE id
- [x] A6. `agent_modes.rs` `update_mode(conn, row)` — UPDATE（禁止修改 is_builtin / kind）— 注：Rust 层 update_mode 本身无限制，保护逻辑在 cmd.rs save_agent_mode 中对内置模式强制保留 name/kind/presentation_rank/is_builtin
- [x] A7. `agent_modes.rs` `delete_mode(conn, id)` — DELETE（is_builtin=true 时返回错误）
- [x] A8. `agent_modes.rs` `seed_builtin_modes(conn)` — 4 个内置预设（默认/快速/深度 + 翻译助手），INSERT OR IGNORE
- [x] A9. `agent_modes.rs` `get_conversation_token_usage(conn, conv_id)` — SUM(messages.tokens) + context_window 计算
- [x] A10. `assistant-storage/src/lib.rs` 新增 `pub mod agent_modes`
- [x] A11. `db.rs` `init_db` 新增 agent_modes 表 CREATE + conversations.mode_id 列 ALTER（ADD COLUMN IF NOT EXISTS，兼容已有数据库）
- [x] A12. `db.rs` `init_db` seed 调用 `agent_modes::seed_builtin_modes`
- [x] A13. 删除 `assistant-storage/src/templates.rs`，`lib.rs` 积除 `pub mod templates`
- [x] A14. `db.rs` `init_db` 移除 `prompt_templates` 表 CREATE（数据库手动 DROP 已执行）

---

## 阶段 B — Rust 后端：Tauri Commands

- [x] B1. `cmd.rs` 新增 `list_agent_modes(app) → Result<Value, String>` — 调用 `agent_modes::get_modes`，返回 JSON
- [x] B2. `cmd.rs` 新增 `save_agent_mode(app, mode: Value) → Result<(), String>` — 解析 JSON → insert 或 update，内置模式仅允许修改 prompt/temperature/max_tokens/model/tools/description
- [x] B3. `cmd.rs` 新增 `delete_agent_mode(app, id: String) → Result<(), String>` — 调用 `agent_modes::delete_mode`
- [x] B4. `cmd.rs` 新增 `get_conversation_token_usage_cmd(app, conversation_id: String) → Result<Value, String>` — 调用 `agent_modes::get_conversation_token_usage`
- [x] B5. `cmd.rs` `send_chat_message` 新增 `mode_id: Option<String>` 参数 — 读取模式配置，注入 system_prompt / temperature / max_tokens
- [x] B6. `cmd.rs` `send_chat_message` system_prompt 注入：非空时在 chat_messages 前方插入 `ChatMessage { role: System, content: system_prompt }`
- [x] B7. `cmd.rs` `send_chat_message` temperature/max_tokens 从模式读取，null 时使用默认值（0.7 / 4096）
- [x] B8. `cmd.rs` `create_conversation` 新增 `mode_id: Option<String>` 参数 — 写入 conversations.mode_id
- [x] B9. `cmd.rs` 删除 `list_prompt_templates` / `save_prompt_template` command
- [x] B10. `lib.rs` invoke_handler 移除模板相关命令注册

---

## 阶段 C — 前端：Store + 类型

- [x] C1. `src/stores/agentModeStore.ts` — Zustand store：`modes` / `selectedModeId` / `loading` / `loadModes` / `selectMode` / `saveMode` / `deleteMode` / `resetBuiltinMode`
- [x] C2. `agentModeStore.ts` `loadModes` — invoke `list_agent_modes` → set modes
- [x] C3. `agentModeStore.ts` `saveMode` — invoke `save_agent_mode` → loadModes 刷新
- [x] C4. `agentModeStore.ts` `deleteMode` — invoke `delete_agent_mode` → loadModes 刷新
- [x] C5. `AgentMode` TypeScript 类型定义（与 Rust AgentModeRow 对齐）

---

## 阶段 D — 前端：chatStore 集成

- [x] D1. `chatStore.ts` 通过静态 import `useAgentModeStore` 读取 `selectedModeId`（注：selectedModeId 归属 agentModeStore 而非 chatStore，职责划分更合理）
- [x] D2. `chatStore.ts` 已有 `contextUsage` 字段（等价 tokenUsage），ContextBar 已消费
- [x] D3. `chatStore.ts` `sendMessage` 传递 `modeId: useAgentModeStore.getState().selectedModeId` 给 `send_chat_message`
- [x] D4. `chatStore.ts` `createConversation` 传递 `modeId` 给 `create_conversation`
- [x] D5. `chatStore.ts` 流式结束时更新 `contextUsage`（已有逻辑，finishStream 中处理 usage）
- [x] D6. 模式选择通过 `agentModeStore.selectMode` 实现（非 chatStore.selectMode，职责归属 agentModeStore）

---

## 阶段 E — 前端：Orb 面板模式选择器

- [x] E1. `src/chat/ModeSelector.tsx` — Ant Design Select，选项从 agentModeStore.modes 读取，按 presentation_rank 排序
- [x] E2. ModeSelector 选项平铺显示 + 名称自描述（注：不采用分组显示，4 个内置 + 少量自定义时分组增加视觉噪音，名称"默认/快速/深度/翻译助手"已自描述无需 Tag）
- [x] E3. ModeSelector 切换时调用 `agentModeStore.selectMode(modeId)`
- [x] E4. ModeSelector 切换时若新模式有 `model` 指定，自动切换模型选择器（ChatView.handleModeChange 遍历 providers 查找匹配）
- [x] E5. ChatView 输入框上方插入 ModeSelector，作为独立 mode bar 行（与顶部 chrome-bar 分离）

---

## 阶段 F — 前端：Token 用量指示

- [x] F1. ContextBar 组件（已有）显示 `used / total` 格式，超 70% 黄色，超 90% 红色（阈值已修正为 0.7 / 0.9）
- [x] F2. ContextBar 数据从 chatStore.contextUsage 读取
- [x] F3. ChatView 底部 ContextBar 已有 token 用量展示，数据源接入 chatStore.contextUsage

---

## 阶段 G — 前端：Agent 模式管理页

- [x] G1. `src/settings/AgentModeManager.tsx` — 列表视图：按分组展示（内置 / 自定义），每行 name + kind Tag + description + 编辑/删除
- [x] G2. AgentModeManager 顶部搜索框 + 新建按钮
- [x] G3. AgentModeManager 编辑 Drawer：name / description / prompt(TextArea) / model(Select) / temperature(Slider 0-2) / max_tokens(InputNumber) / tools 预留
- [x] G4. 内置模式编辑：仅 prompt / temperature / max_tokens / model / description 可编辑，name / kind 只读，无删除按钮；新增"恢复默认"按钮（reset_builtin_mode）
- [x] G5. 自定义模式：全部可编辑 + 可删除（确认弹窗）
- [x] G6. 保存调用 `agentModeStore.saveMode`，删除调用 `agentModeStore.deleteMode`，恢复默认调用 `agentModeStore.resetBuiltinMode`

---

## 阶段 H — 前端：路由 + 侧边栏 + Prompt 模板移除

- [x] H1. App.tsx 新增 `/settings/agents` 路由 → AgentModeManager
- [x] H2. App.tsx 删除 `/settings/templates` 路由（不保留重定向）
- [x] H3. 侧边栏设置菜单"Prompt 模板" → "Agent 模式"，路由指向 `/settings/agents`
- [x] H4. 删除 `src/settings/TemplateManager.tsx`
- [x] H5. 确认无其他组件引用 TemplateManager / list_prompt_templates / save_prompt_template

---

## 阶段 I — 集成验证

### I1. 编译验证

- [x] `cargo check` 通过
- [x] `tsc --noEmit` 通过
- [x] `npm run lint` 0 errors（1 warning 已有，非 I6 引入）

**依赖**：A1~A12, B1~B10, C1~C5, D1~D6, E1~E5, F1~F3, G1~G6, H1~H5

---

### I2. 功能验证

- [x] `agent_modes` 表创建成功，4 个内置预设自动 seed
- [x] `conversations` 表新增 `mode_id` 列
- [x] 列出所有 Agent 模式（内置 + 自定义），按 presentation_rank 排序
- [x] 创建自定义 Agent 模式（name + prompt + temperature + max_tokens + model）
- [x] 编辑内置模式的 prompt / temperature / max_tokens
- [x] 删除自定义模式成功
- [x] 删除内置模式返回错误
- [x] Orb 面板模式选择器可切换模式，默认选中"默认"模式
- [x] 选中"默认/快速/深度"模式时，不注入 system prompt，仅使用对应的 temperature / max_tokens
- [x] 选中"翻译助手"时，system prompt 注入对话
- [x] 不选模式时发送消息，使用默认值（temperature=0.7, max_tokens=4096, 无 system prompt）
- [x] `/settings/agents` 页面可管理所有模式
- [x] `/settings/templates` 路由已删除
- [x] 侧边栏显示"Agent 模式"
- [x] token 用量在 Orb 面板底部 ContextBar 显示
- [x] token 用量超 70% 显示黄色警告
- [x] token 用量超 90% 显示红色警告
- [x] `prompt_templates` 表已 DROP，`templates.rs` 已删除

**依赖**：I1

---

### I3. 验收标准

- [x] I6-spec §7 验收标准 1~16 项全部通过

**依赖**：I2
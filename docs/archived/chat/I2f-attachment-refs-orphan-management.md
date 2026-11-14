# I2f — 附件关联索引与孤儿文件管理设计

> **前置**：I2e Phase 1 ✅（图片附件完整链路：ContentBlock[] JSON 存储 + AttachmentStore 磁盘存储 + admit/rehydrate/project）
> **性质**：纯分析文档，描述附件关联索引的引入动机、数据模型、生命周期策略与管理页面设计
> **核心决策**：附件单独存储、面向会话索引、允许孤儿文件、不做额外自动删除；主功能为删除指定附件，按会话批量删除为便利功能，一键清理孤儿为备选功能

---

## 1. 动机：为什么需要关联索引

### 1.1 当前问题

I2e Phase 1 完成后，附件存储链路为：

```
用户发送图片 → admit_attachments() → 写磁盘 attachments/<sha256>.png
             → insert_message(content)  ← content 含 ImageBlock { attachmentId: "sha256.png" }
```

**缺失环节**：

| 问题 | 现状 | 影响 |
|------|------|------|
| 删除会话不清理附件 | `delete_conversation()` 只删 `messages` + `conversations` 表 | 磁盘附件成为孤儿，永不清除 |
| 无法按会话查询附件占用 | 无会话-附件关联 | 用户无法知道"这个会话占了多少空间" |
| 无法识别孤儿文件 | 无引用计数机制 | 无法区分"有会话引用的附件"和"孤儿附件" |
| 无法统计存储分布 | 无元数据索引 | 无法回答"图片占多少、文件占多少、总计多少" |

### 1.2 设计约束

根据项目决策，本次设计遵循以下约束：

1. **附件单独存储**：附件字节不进 SQLite，仅存磁盘 `attachments/` 目录（与 I2e 一致）
2. **面向会话索引**：通过关联表建立 `attachment ↔ conversation` 索引，支持按会话查询占用
3. **允许孤儿文件**：删除会话时不级联删除磁盘附件，孤儿文件合法存在
4. **不做额外自动删除**：不引入 TTL 过期清扫、不引入 LRU 驱逐、不引入启动时自动清理
5. **用户主动管理**：用户可在管理页面删除指定附件；按会话批量删除为便利功能，一键清理孤儿为备选功能

---

## 2. 数据模型

### 2.1 关联表 `attachment_refs`

在 [db.rs](../../../src-tauri/crates/assistant-storage/src/db.rs) 的 `init_db()` 中新增：

```sql
CREATE TABLE IF NOT EXISTS attachment_refs (
    attachment_id   TEXT NOT NULL,
    conversation_id TEXT NOT NULL,
    message_id      TEXT NOT NULL,
    PRIMARY KEY (attachment_id, conversation_id, message_id)
);

CREATE INDEX IF NOT EXISTS idx_attachment_refs_conv
    ON attachment_refs(conversation_id);

CREATE INDEX IF NOT EXISTS idx_attachment_refs_att
    ON attachment_refs(attachment_id);
```

**字段说明**：

| 字段 | 类型 | 语义 |
|------|------|------|
| `attachment_id` | TEXT | 磁盘文件名，如 `a1b2c3d4e5f6.png`（SHA-256 + 扩展名） |
| `conversation_id` | TEXT | 引用该附件的会话 ID |
| `message_id` | TEXT | 引用该附件的消息 ID |

> **为什么只存三列**：`media_type` / `bytes` / `created_at` 可从磁盘 `AttachmentStore::list_all()` 获取，无需冗余存储。`get_storage_breakdown` 查询关联表得到 attachment_id 列表后，与 `list_all()` 的结果做 join 即可补全附件元数据，而 `list_all()` 本来就要调（用于孤儿判定），无额外开销。

**联合主键** `(attachment_id, conversation_id, message_id)` 的含义：
- 同一附件可被同一会话的多条消息引用 → 多行
- 同一附件可被不同会话引用 → 多行（内容寻址去重，磁盘仅一份）

**索引用途**：
- `idx_attachment_refs_conv`：按会话查询附件列表 / 统计会话占用
- `idx_attachment_refs_att`：查询某附件的引用计数（判断是否孤儿）

### 2.2 与现有表的关系

```
conversations          messages               attachment_refs        磁盘
┌──────────────┐     ┌──────────────┐      ┌──────────────────┐    ┌──────────────┐
│ id (PK)      │←────│ conversation_id│     │ conversation_id  │    │              │
│ title        │     │ id (PK)       │←────│ message_id       │    │ <sha256>.png │
│ ...          │     │ content       │     │ attachment_id    │───→│ <sha256>.pdf │
└──────────────┘     │ ...           │     └──────────────────┘    │ ...          │
                     └──────────────┘                              └──────────────┘
```

**关键区别**：`attachment_refs` 不使用 `REFERENCES ... ON DELETE CASCADE`，因为：
- 删除会话时 **故意不删** `attachment_refs` 中该会话的记录 → 孤儿文件合法存在
- 孤儿判定逻辑：磁盘文件存在 **且** `attachment_refs` 中无任何引用
- 用户在管理页面主动清理时，才删除孤儿文件

### 2.3 孤儿文件的定义

```
孤儿文件 = 磁盘文件存在
         AND NOT EXISTS (
             SELECT 1 FROM attachment_refs
             WHERE attachment_id = <该文件的 attachment_id>
         )
```

**产生场景**：

| 场景 | 产生原因 |
|------|---------|
| 删除会话 | 显式 `DELETE FROM attachment_refs WHERE conversation_id = ?` 删关联行，但磁盘文件保留 |
| 删除单条消息 | 消息被删除，`attachment_refs` 中该消息的行被删，磁盘文件保留 |
| 数据库损坏/重置 | 磁盘文件存在但数据库中无任何记录 |
| 程序异常中断 | `admit_attachments()` 已写磁盘但 `insert_message()` 未完成，`attachment_refs` 无记录 |

---

## 3. 生命周期策略

### 3.1 入库（Ingress）— 写关联

**触发点**：[chat.rs](../../src-tauri/src/cmd/chat.rs) `send_chat_message()` 中 `insert_message()` 之后

**当前流程**：
```
content → admit_attachments() → 写磁盘 → insert_message(content)
```

**新增步骤**：
```
content → admit_attachments() → 写磁盘 → insert_message(content)
                                                    ↓
                              insert_attachment_refs(conn, conversation_id, message_id, content)
                                                    ↓
                              遍历 ContentBlock[] 中的 ImageBlock / FileBlock
                              → INSERT INTO attachment_refs
```

**`insert_attachment_refs` 逻辑描述**：

1. 将 `content` 反序列化为 `Vec<ContentBlock>`
2. 遍历每个 block，仅处理含 `attachmentId` 的变体（ImageBlock / FileBlock），提取 `attachment_id`
3. 对每个提取结果执行 `INSERT OR IGNORE INTO attachment_refs`，联合主键冲突时静默跳过（同一附件被同一消息重复引用）

### 3.2 删除会话 — 不级联删附件

**触发点**：[conversation.rs](../../../src-tauri/src/cmd/conversation.rs) 的三个删除函数

**当前行为**（[crud.rs](../../../src-tauri/crates/assistant-storage/src/conversations/crud.rs) `delete_conversation()`）：先 `DELETE FROM messages WHERE conversation_id = ?`，再 `DELETE FROM conversations WHERE id = ?`，不涉及附件。

**设计决策：不做额外删除**。删除会话时，在同一事务中依次执行：
1. `DELETE FROM conversations WHERE id = ?` — 删会话元数据（`messages` 由 `ON DELETE CASCADE` 自动级联删除）
2. `DELETE FROM attachment_refs WHERE conversation_id = ?` — 删关联索引
3. **不删磁盘文件** — 孤儿文件合法存在，交给用户管理

> **事务要求**：以上两步必须在同一个 SQLite 事务中完成，避免中途失败导致 `attachment_refs` 残留悬空引用（DB 有行但对应 conversation 已删）。

同理适用于 `batch_delete` 和 `purge_all`。

**为什么允许孤儿文件？**
- 用户可能误删会话后想恢复（未来可能的"回收站"功能）
- 自动删除不可逆，用户数据安全优先
- 孤儿文件不影响功能运行（仅占磁盘空间）
- 微信也采用相同策略：数据库记录删除后文件可能残留，用户可手动清理

### 3.3 删除单条消息 — 删关联索引

当用户删除单条消息时（如分支操作），需要同步删除 `attachment_refs` 中该消息的行：`DELETE FROM attachment_refs WHERE message_id = ?`。磁盘文件保留 → 可能产生孤儿文件 → 交给用户管理。

### 3.4 孤儿文件清理 — 备选功能

**不在任何自动流程中触发**。用户在管理页面展开"孤儿附件"分组，可勾选单个或全选后删除。前端从 `StorageBreakdownDto.orphans` 中获取孤儿 attachment_id 列表，调用 `delete_attachments(ids)` 执行删除。

孤儿集合的计算由 `get_storage_breakdown` 完成：`list_all()` 磁盘文件集合 − `SELECT DISTINCT attachment_id FROM attachment_refs` 有引用集合 = 孤儿集合。

---

## 4. 管理页面设计

### 4.1 页面位置

在设置系统中新增 **"存储管理"** 页面，注册到 [pageRegistry.ts](../../../src/settings/core/pageRegistry.ts) 的 `SETTINGS_PAGES` 数组中，`categoryId: "general"`，使用 `DatabaseOutlined` 图标。

### 4.2 页面结构

页面为**一个分组可展开列表**，每个分组可展开显示其下的单个附件，支持在任意层级勾选后批量删除。

列表分组顺序：
1. **第一组：孤儿附件** — 分组标题显示"无关联对话的附件"、孤儿数量和占用空间。展开后罗列每个孤儿附件（文件名、大小、创建时间），可勾选单个或全选
2. **后续组：有附件的对话** — 分组标题显示会话标题、附件数量、占用空间、最近活跃时间，按占用空间降序排列。展开后罗列该会话下的每个附件（文件名、大小、创建时间），可勾选单个或全选

概览区在列表上方显示附件总数和总占用空间。

所有删除操作均需弹出确认弹窗，提示操作不可逆。

### 4.3 功能层级

| 层级 | 功能 | 说明 |
|------|------|------|
| **主功能** | 删除指定附件 | 勾选单个附件行 → 收集 attachment_id → 调用 `delete_attachments` |
| **便利功能** | 按分组批量删除 | 勾选分组标题行（对话或孤儿组）→ 收集该组所有 attachment_id → 调用同一个 `delete_attachments` |

### 4.4 Rust 侧 DTO

- **`StorageBreakdownDto`**：`total_bytes: u64`、`total_count: u64`、`orphans: Vec<AttachmentSummary>`、`conversations: Vec<ConversationAttachmentSummary>`
- **`AttachmentSummary`**：`attachment_id: String`、`media_type: String`、`bytes: u64`、`created_at: i64`
- **`ConversationAttachmentSummary`**：`conversation_id: String`、`title: String`、`attachments: Vec<AttachmentSummary>`、`total_bytes: u64`、`updated_at: i64`
- **`AttachmentDeleteResultDto`**：`removed_count: u64`、`removed_bytes: u64`

> `AttachmentSummary` 同时用于孤儿组和对话组内的附件罗列，前端展开分组时直接渲染，无需二次查询。孤儿数量和占用空间由前端从 `orphans` 派生（`orphans.length` / `orphans.reduce(sum bytes)`），无需后端单独返回。

### 4.5 Tauri Command

新增一个 Tauri command 文件 `storage.rs`，包含两个独立 command：

**`get_storage_breakdown(app) → StorageBreakdownDto`** — 查询存储分布（概览 + 孤儿附件列表 + 各对话附件列表）

**`delete_attachments(app, attachment_ids: Vec<String>) → AttachmentDeleteResultDto`** — 删除指定附件

对每个 `attachment_id`：删除磁盘文件 + 删除 `attachment_refs` 中所有该 `attachment_id` 的行。

前端勾选附件行（单个或分组全选）后收集 attachment_id 列表调用此 command。

### 4.6 UI 风格适配要求

前端管理页面**必须**遵循项目现有设置页面的设计规范，不得引入与全局风格不一致的组件或布局。具体要求：

#### 4.6.0 设置总览展示

- 存储管理页面为非表单驱动页面（数据来自 Tauri command，非 conf store），与 ProviderManager / AgentModeSettings 同模式：
  - **不注册** `defineSection` / `defineSetting`
  - 总览中走 `NavOnlySummary` 路径：仅显示图标 + 标题，点击跳转到详情页
  - 详情页内**不使用** `SettingsSection` 组件（`SettingsSection` 依赖 `defineSection` 注册才能获取标题），自行组织分区标题
- 存储占用详情仅在详情页内展示，无需扩展总览的自定义摘要机制

#### 4.6.1 页面骨架

- 使用 `SettingsPageShell`，根据内容宽度选择 `wide` 参数：
  - 默认 `wide=false`（`maxWidth: 700px` 居中布局），适用于内容较窄的页面（如 GeneralSettings、AboutSettings）
  - `wide`（即 `wide=true`，无 maxWidth 限制），适用于含列表/表格等宽内容的页面（如 ProviderManager、AgentModeSettings）
  - `wide={N}`（自定义 maxWidth），如 ShortcutSettings 用 `wide={600}`
  - 存储管理页面含折叠分组列表，建议使用 `wide`（无宽度限制），与 ProviderManager / AgentModeSettings 一致
- `SettingsPageShell` 自动提供返回按钮 + 页面标题 + 分割线，页面组件内**不再重复**渲染标题

#### 4.6.2 页面内容布局

- 页面内容分两部分，**不使用分区标题**（与 ProviderManager 同模式：顶部操作区 + 列表区，无 `Typography.Title` 分隔）：
  1. 概览摘要行（`Typography.Text` + 操作按钮）
  2. 折叠分组列表区
- 两区之间用 `marginBottom: 16` 间距自然过渡，无需额外分隔

#### 4.6.3 概览区

- 使用 `Typography.Text` 渲染一行摘要文字（如"共 128 个附件，占用 2.3 GB"），轻量简洁
- 操作按钮（删除选中、刷新）使用 `Button size="small"`，与摘要文字同行排列（`Space` 或 `Flex justify="space-between"`）

#### 4.6.4 折叠分组列表

- 采用与历史对话管理（[HistorySidebar](../../../src/history/sidebar/HistorySidebar.tsx)）一致的轻量自定义折叠风格，而非 antd `Collapse` 卡片式折叠（antd `Collapse` 自带边框/背景/圆角的卡片样式，视觉重量过重，与项目内列表折叠风格不一致）
  - 分组头使用 `RightOutlined` / `DownOutlined` 箭头图标表示折叠/展开状态
  - 折叠状态由 `collapsedGroups: Set<string>` 管理，点击分组头切换
  - 分组头样式使用 CSS Module，参考 [HistorySidebar.module.css](../../../src/history/sidebar/HistorySidebar.module.css) 的 `.groupHeader` / `.groupArrow` / `.groupLabel` / `.groupCount` 定义：
    ```css
    .groupHeader { display:flex; align-items:center; padding:4px 10px; cursor:pointer; user-select:none; color:var(--color-text-tertiary); font-size:11px; font-weight:500; }
    .groupHeader:hover { background:var(--color-border-secondary); border-radius:4px; }
    .groupArrow { width:12px; flex-shrink:0; font-size:10px; }
    .groupLabel { flex:1; }
    .groupCount { font-size:10px; opacity:0.6; }
    ```
  - 展开内容区缩进至箭头右侧对齐（padding-left 与箭头宽度对齐）

#### 4.6.5 勾选交互

- 分组标题行包含 `Checkbox`（全选/半选），与箭头同行排列
- 分组标题行的 Checkbox `onClick` 必须 `e.stopPropagation()` 防止触发展开/折叠
- 单条附件行包含 `Checkbox`，行 hover 时显示浅背景（`var(--color-border-secondary)`）

#### 4.6.6 i18n

- 所有用户可见文字必须通过 i18n key 引用，**禁止**硬编码中/英文字符串
- key 遵循 `settings.storage.*` 命名空间，与现有 `settings.provider.*` / `settings.agent_mode.*` 一致
- 导航标题 key 为 `settings.storage.title`

#### 4.6.7 类型安全

- Tauri command 调用必须通过 `storageService.ts` 服务层封装，返回类型与 Rust DTO 一一对应
- **禁止**使用 `as unknown as` 双重断言（项目规范仅允许 2 处特例）
- **禁止**用 `Record<string, unknown>` 传递部分字段

### 4.7 交互规则

| 操作 | 行为 |
|------|------|
| 页面加载 | 调用 `get_storage_breakdown()` 获取数据，渲染分组列表 |
| 展开分组 | 直接渲染 DTO 中已包含的附件列表，无需二次请求 |
| 勾选单个附件 | 收集该附件 attachment_id |
| 勾选分组标题 | 收集该组所有附件 attachment_id |
| 删除 | 确认弹窗 → 调用 `delete_attachments(attachment_ids)` → 刷新 |
| 孤儿为 0 | "孤儿附件"分组不显示 |

---

## 5. 涉及修改的文件清单

### 5.1 Rust — Schema 层

| 文件 | 修改 |
|------|------|
| `assistant-storage/src/db.rs` | `init_db()` 新增 `attachment_refs` 表 + 两个索引 |

### 5.2 Rust — Storage 层

| 文件 | 修改 |
|------|------|
| `assistant-storage/src/attachment_store.rs` | 新增关联表的 CRUD 函数（insert / delete / query） |
| `assistant-storage/src/conversations/crud.rs` | `delete_conversation()` 中追加 `DELETE FROM attachment_refs WHERE conversation_id = ?` |
| `assistant-storage/src/conversations/ops.rs` | `batch_delete()` / `purge_all()` 中追加删 `attachment_refs`；`duplicate_conversation()` 中追加复制 `attachment_refs` 行 |

### 5.3 Rust — Agent 层

| 文件 | 修改 |
|------|------|
| `assistant-agent/src/multimodal.rs` | `admit_attachments()` 返回值扩展，或由调用方在 `insert_message` 后调用 `insert_refs()` |

### 5.4 Rust — Command 层

| 文件 | 修改 |
|------|------|
| `src-tauri/src/cmd/chat.rs` | `send_chat_message()` 中 `insert_message()` 后调用 `insert_attachment_refs()` |
| `src-tauri/src/cmd/conversation.rs` | `delete_conversation()` / `batch_delete_conversations()` / `purge_conversations()` 中追加删 `attachment_refs` |
| `src-tauri/src/cmd/` 新增 `storage.rs` | `get_storage_breakdown()` + `delete_attachments(attachment_ids)` 两个 command |

### 5.5 Rust — Setup 层

| 文件 | 修改 |
|------|------|
| `src-tauri/src/setup.rs` | `invoke_handler` 注册新增的 storage commands |

### 5.6 前端

| 文件 | 修改 |
|------|------|
| `settings/core/pageRegistry.ts` | 新增 `storage` 页面定义 |
| `settings/pages/StorageSettings.tsx` | 新增：存储管理页面组件 |
| `services/storageService.ts` | 新增：调用 Tauri command 的服务层 |
| i18n 资源文件 | 新增翻译 key |

---

## 6. 数据流全景

```
  入库 (send_chat_message)
  ─────────────────────
  content ──→ admit_attachments()
                 ├─ ImageBlock: save_image() → 写磁盘 → 返回 attachmentId
                 └─ FileBlock:  save_file()  → 写磁盘 → 返回 attachmentId
                 ↓
             insert_message(content)    ← content 含 ContentBlock[] JSON
                 ↓
             insert_attachment_refs()   ← ★ 新增：遍历 ContentBlock[] 写关联表
                 │
                 │  attachment_refs:
                 │  ┌──────────────┬──────────────────┬──────────────┐
                 │  │ attachment_id│ conversation_id  │ message_id   │
                 │  │ abc123.png   │ conv-001         │ msg-001      │
                 │  │ abc123.png   │ conv-002         │ msg-005      │
                 │  │ def456.pdf   │ conv-001         │ msg-002      │
                 │  └──────────────┴──────────────────┴──────────────┘

  删除会话 (delete_conversation)
  ─────────────────────────────
  1. DELETE FROM conversations WHERE id = conv-001  ← messages 由 CASCADE 自动删
  2. DELETE FROM attachment_refs WHERE conversation_id = conv-001  ← ★ 新增
  3. 磁盘文件保留不动 ← 允许孤儿

  孤儿判定
  ────────
  1. list_all() → 磁盘文件 ID 集合
  2. SELECT DISTINCT attachment_id FROM attachment_refs → 有引用的 ID 集合
  3. 差集 = 磁盘集合 − 有引用集合 → 孤儿文件

  用户删除附件 (delete_attachments)
  ─────────────────────────────────
  前端勾选条目 → 展开为 attachment_ids
  → delete_attachments(attachment_ids)
  → 对每个 id：删磁盘文件 + DELETE FROM attachment_refs WHERE attachment_id = ?
```

---

## 7. 与微信设计对比

微信用物理目录分桶（`MsgAttach/<msgHash>/`）实现按会话索引，删除会话时文件可能残留（"文件已过期"），用户手动清理存储空间。

本项目用关联表 `attachment_refs` 实现逻辑索引——因为内容寻址（SHA-256）天然去重，同一文件可被多会话引用，物理分桶会破坏去重。关联表更灵活且不破坏存储层的简洁性。

---

## 8. 安全与边界

| 关注点 | 处理 |
|--------|------|
| `attachment_refs` 与磁盘不一致 | 孤儿判定仅看"磁盘有 + DB 无"，不处理"DB 有 + 磁盘无"（悬空引用由前端 `image_not_found_placeholder` 降级处理） |
| 清理操作不可逆 | UI 弹确认弹窗，明确提示"清理后不可恢复" |
| 清理期间并发写入 | `delete_attachments` 接收前端已确定的 attachment_id 列表，按 id 逐个删除磁盘文件和索引行，操作原子性由单文件删除保证；若删除期间新消息引用了同一附件，该附件已被删无法恢复，但前端已弹确认弹窗提示不可逆 |
| 大量附件扫描性能 | 一次 `SELECT DISTINCT attachment_id FROM attachment_refs` + 一次 `list_all()` 目录遍历 + 内存集合差集，避免逐文件 COUNT(*) 的 N+1 问题 |
| 磁盘满 | 不做自动驱逐，用户在管理页面看到总占用后自行决定清理 |
| `duplicate_conversation` | 复制会话时须同步复制 `attachment_refs` 行（新 conversation_id + 新 message_id），否则复制会话的附件在存储统计中不可见，且原会话删除后引用关系断裂 |
| `purge_all` 后附件状态 | 清空所有会话后全部附件变为孤儿，不自动清理磁盘，用户需在管理页面手动清理 |
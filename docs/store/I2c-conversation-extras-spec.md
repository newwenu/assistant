# I2c — 对话扩展功能规格说明

> **来源**：从 I2-History §2.5 / §3.1 / §3.4 / §4.3 / §4.9.3 / §4.10 / §4.12 / §5.2 / §5.3 / §6.1 / §6.2 暂缓内容提取
> **前置**：I2-History ✅ 已完成

## 1. 目标

实现对话级别的扩展功能，提升组织和管理效率：
- **对话视觉标识**：颜色标记 + 标签分类
- **草稿保存/恢复**：每对话独立输入框草稿
- **冗余统计缓存**：message_count / total_tokens
- **对话导入**：JSON 导入 + ID 冲突处理
- **AI 标题生成**：轻量模型自动生成对话标题
- **侧栏增强**：多选模式 + 键盘导航
- **TTS 语音朗读**：Web Speech API
- **流式速率指示**：tokens/s 显示

---

## 2. 对话视觉标识

### 2.1 颜色标记

```typescript
interface Conversation {
  // ... 现有字段 ...
  color_mark: string | null   // 如 "red" / "blue" / "green" 等
}
```

**Rust 端**：

```sql
ALTER TABLE conversations ADD COLUMN color_mark TEXT;
```

```rust
#[command]
pub fn set_conversation_color_mark(app: AppHandle, id: String, color: String) -> Result<(), String>
```

**UI**：侧栏对话项显示颜色圆点；右键菜单 → 颜色标记子菜单。

### 2.2 标签分类

```typescript
interface Conversation {
  // ... 现有字段 ...
  labels: string[]
}
```

**Rust 端**：

```sql
ALTER TABLE conversations ADD COLUMN labels TEXT NOT NULL DEFAULT '[]';

CREATE TABLE IF NOT EXISTS conversation_labels (
  conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
  label TEXT NOT NULL,
  PRIMARY KEY (conversation_id, label)
);
```

```rust
#[command]
pub fn set_conversation_labels(app: AppHandle, id: String, labels: Vec<String>) -> Result<(), String>
```

**UI**：右键菜单 → 标签管理子菜单；侧栏可按标签过滤。

### 2.3 暂缓理由

颜色标记和标签使用频率低，当前对话数量少，分组和搜索已满足需求。

---

## 3. 草稿保存/恢复

### 3.1 问题

当前 `MessageInput` 使用 `useState("")` 管理输入框内容，因组件切换对话时不卸载（无 `key={conversationId}`），输入框内容在切换时**不会丢失但会"串台"**——对话 A 输入半截切到 B，B 的输入框会显示 A 的半截内容。

### 3.2 设计

```typescript
interface Conversation {
  // ... 现有字段 ...
  draft_content: string | null
  draft_updated_at: number | null
}
```

**Rust 端**：

```sql
ALTER TABLE conversations ADD COLUMN draft_content TEXT;
ALTER TABLE conversations ADD COLUMN draft_updated_at INTEGER;
```

```rust
pub fn save_draft(conn: &Connection, id: &str, content: &str) -> Result<()> {
    let now = now_epoch_ms();
    conn.execute(
        "UPDATE conversations SET draft_content = ?1, draft_updated_at = ?2 WHERE id = ?3",
        params![content, now, id],
    )?;
    Ok(())
}

#[command]
pub fn save_conversation_draft(app: AppHandle, id: String, content: String) -> Result<(), String>
```

**前端 chatStore**：

```typescript
interface ChatState {
  saveDraft: (content: string) => Promise<void>
}

// switchConversation 中：
// 1. 保存当前对话草稿：saveDraft(currentInputContent)
// 2. 切换到新对话
// 3. 从 conversation.draft_content 恢复输入框内容
```

### 3.3 暂缓理由

输入框"串台"问题使用频率低，用户通常不会在多个对话间频繁切换且保留半截输入。

---

## 5. 冗余统计缓存

### 5.1 设计

```typescript
interface Conversation {
  // ... 现有字段 ...
  message_count: number
  total_tokens: number
}
```

**Rust 端**：

```sql
ALTER TABLE conversations ADD COLUMN message_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE conversations ADD COLUMN total_tokens INTEGER NOT NULL DEFAULT 0;
```

```rust
pub fn update_conversation_stats(conn: &Connection, id: &str) -> Result<()> {
    conn.execute(
        "UPDATE conversations SET
           message_count = (SELECT COUNT(*) FROM messages WHERE conversation_id = ?1),
           total_tokens = (SELECT COALESCE(SUM(tokens), 0) FROM messages WHERE conversation_id = ?1)
         WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}
```

每次增删消息后同步更新。

### 5.2 暂缓理由

冗余缓存增加维护负担，需每次增删消息同步更新。分页加载的 `get_message_count` 已用 `COUNT(*)` 实时查询，性能足够。

---

## 6. 对话导入

### 6.1 设计

```rust
#[command]
pub fn import_conversation(app: AppHandle, data: String, format: String) -> Result<String, String>
```

- `format`：`"json"`
- 导入 JSON 格式的对话数据
- ID 冲突处理：生成新 ID
- 格式校验 + 版本兼容

### 6.2 暂缓理由

导入需要格式校验、版本兼容、ID 冲突处理，复杂度高但使用频率极低。导出功能已实现。

---

## 7. AI 标题生成

### 7.1 设计

```rust
#[command]
pub async fn generate_conversation_title(
    app: AppHandle,
    conversation_id: String,
) -> Result<(), String>
```

**流程**：
1. 检查 `title_source`：若为 `user` 或 `ai`，跳过（不覆盖）
2. 获取首条用户消息 + 首条助手回复
3. 若无助手回复 → fallback 标题
4. 若有助手回复 → 调用轻量模型（temperature=0.2）生成标题
5. AI 生成失败/空 → fallback 标题
6. AI 生成成功 → `sanitize_generated_title`（去引号、去 markdown、限制长度）→ 更新标题 + `title_source = 'ai'`

**System Prompt**（参考 openhuman/OpenBitFun）：
```
You are a conversation title generation assistant. Based on the user's message, generate a concise title.
Requirements:
- At most 5 words
- No quotes, no markdown
- Return only the title text
```

**User Prompt**：
```
First user message:
{user_message}

Assistant reply:
{assistant_message}

Return the best conversation title.
```

**触发时机**：`finishStream` 中，若 `title_source === 'fallback'`，fire-and-forget 调用 `generate_conversation_title`。

### 7.2 暂缓理由

需要 Rust 端 `generate_conversation_title` command + 轻量模型调用链路，当前 fallback 已满足基本需求。AI 标题生成作为体验增强，优先级低于核心历史管理功能。

---

## 8. 侧栏增强

### 8.1 多选模式

- 侧栏顶部增加 ☑ 按钮，点击进入多选模式
- 多选模式：每项前显示 Checkbox，底部显示操作栏（批量删除/批量导出/批量置顶）
- `ChatState` 新增：`multiSelectMode: boolean` / `selectedConversationIds: Set<ConversationId>`
- Escape 退出多选模式

**暂缓理由**：复杂 UI 交互，使用频率低，批量删除用 Shift 多选或右键即可。

### 8.2 键盘导航

| 快捷键 | 操作 |
|--------|------|
| ↑ / ↓ | 上下移动选中 |
| Enter | 打开选中对话 |
| Delete | 删除选中对话 |
| Escape | 退出搜索/多选 |
| Ctrl+F | 聚焦搜索框 |

**暂缓理由**：侧栏键盘导航是桌面应用的锦上添花，不是核心需求。

---

## 9. TTS 语音朗读

### 9.1 设计

- 助手消息操作栏增加 🔊 朗读按钮
- 使用 Web Speech API `speechSynthesis.speak()`
- 朗读中按钮变为 ⏹ 停止
- 支持 `speechSynthesis.getVoices()` 选择语音

### 9.2 暂缓理由

Web Speech API 兼容性差、语音质量参差、中英文混合效果不好，投入产出比低。

---

## 10. 流式速率指示

### 10.1 设计

- 流式中在助手消息底部显示 `XX tokens/s`
- 计算：`completion_tokens / (now - streamStartTime) * 1000`

### 10.2 暂缓理由

信息价值低，用户不关心速率数字，关心的是内容本身。

---

## 11. 实施步骤

### Step 1：颜色标记 + 标签

1. Rust 端：`conversations` 表新增 `color_mark` / `labels` 列 + `conversation_labels` 表
2. Rust 端：新增 `set_conversation_color_mark` / `set_conversation_labels` command
3. 前端：`Conversation` 接口新增字段
4. 前端：侧栏右键菜单扩展 + 颜色圆点 + 标签过滤

### Step 2：草稿保存/恢复

1. Rust 端：`conversations` 表新增 `draft_content` / `draft_updated_at` 列
2. Rust 端：新增 `save_conversation_draft` command
3. 前端：`switchConversation` 中保存/恢复草稿

### Step 3：冗余统计缓存

1. Rust 端：`conversations` 表新增 `message_count` / `total_tokens` 列
2. Rust 端：新增 `update_conversation_stats` 函数，增删消息后调用
3. 前端：`Conversation` 接口新增字段

### Step 5：对话导入

1. Rust 端：新增 `import_conversation` command（JSON 解析 + ID 冲突处理）
2. 前端：导入 UI（文件选择 + 格式选择）

### Step 6：AI 标题生成

1. Rust 端：新增 `generate_conversation_title` command
2. 前端：`finishStream` 中触发 AI 标题生成

### Step 7：侧栏增强（多选 + 键盘导航）

1. 前端：多选模式 UI + 状态管理
2. 前端：键盘导航事件处理

### Step 7：TTS + 流式速率

1. 前端：Web Speech API 集成
2. 前端：流式速率计算 + 显示

---

## 12. 验收标准

- [ ] **颜色标记**：右键菜单设置颜色 → 侧栏显示颜色圆点
- [ ] **标签**：右键菜单管理标签 → 侧栏按标签过滤
- [ ] **草稿保存**：输入框输入内容 → 切换对话 → 切回 → 内容恢复
- [ ] **统计缓存**：`message_count` / `total_tokens` 与实际一致
- [ ] **对话导入**：导出 JSON → 删除对话 → 导入 → 对话恢复
- [ ] **AI 标题**：`title_source='fallback'` → AI 生成成功 → `title_source='ai'`
- [ ] **多选模式**：☑ 按钮 → Checkbox → 批量操作
- [ ] **键盘导航**：↑↓ / Enter / Delete / Escape 快捷键生效
- [ ] **TTS**：🔊 朗读 → 语音播放 → ⏹ 停止
- [ ] **流式速率**：流式中显示 `XX tokens/s`
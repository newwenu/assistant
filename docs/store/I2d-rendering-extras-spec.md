# I2d — 渲染增强规格说明

> **来源**：从 I2-History §4.10 / §4.11 / §6.3 暂缓内容提取
> **前置**：I2-History ✅ 已完成

## 1. 目标

提升消息渲染性能和体验：
- **代码块语法高亮**：集成 `shiki`，支持 100+ 语言 + 语言标签 + 复制按钮
- **虚拟滚动**：长对话性能优化，`react-virtuoso` 集成
- **客户端自动重试**：`retryWithBackoff` 指数退避，可重试错误自动重发

---

## 2. 代码块语法高亮

### 2.1 设计

集成 `shiki`（与 airi 一致），替换当前 `react-markdown` 默认代码块渲染。

**依赖**：`npm install shiki`（约 5-10MB，含全部 TextMate 语法 + 主题）

**功能**：
- 100+ 语言语法高亮
- 代码块右上角：语言标签 + 复制按钮 + 行号切换
- 多主题支持（亮色/暗色）

**实现**：

```tsx
// CodeBlock.tsx
import { codeToHtml } from 'shiki'

const CodeBlock: FC<{ language: string; code: string }> = ({ language, code }) => {
  const [html, setHtml] = useState<string>('')

  useEffect(() => {
    codeToHtml(code, {
      lang: language || 'text',
      theme: isDark ? 'github-dark' : 'github-light',
    }).then(setHtml)
  }, [code, language, isDark])

  return (
    <div className={css.wrapper}>
      <div className={css.header}>
        <span className={css.lang}>{language}</span>
        <button onClick={copyCode}>📋</button>
      </div>
      <div dangerouslySetInnerHTML={{ __html: html }} />
    </div>
  )
}
```

**ReactMarkdown 集成**：

```tsx
<ReactMarkdown
  components={{
    code({ className, children }) {
      const lang = className?.replace('language-', '') ?? ''
      if (lang) {
        return <CodeBlock language={lang} code={String(children)} />
      }
      return <code className={className}>{children}</code>
    }
  }}
/>
```

### 2.2 暂缓理由

shiki 含全部 TextMate 语法+主题约 5-10MB，当前阶段核心是存储 CRUD+分页+对话操作，代码高亮非核心，延后到渲染增强阶段再装。

---

## 3. 虚拟滚动

### 3.1 问题

当消息数 > 100 时，DOM 节点过多导致渲染卡顿。分页加载已解决大部分问题（每次只渲染 50 条），但极端场景（用户不滚动一直发消息）仍可能有性能问题。

### 3.2 设计

当消息数 > `VIRTUAL_SCROLL_THRESHOLD`（默认 100）时启用虚拟列表。

| 参数 | 值 | 说明 |
|------|-----|------|
| 库 | `react-virtuoso` | 成熟的 React 虚拟列表，支持分组/跟随/反向无限加载 |
| `VIRTUAL_SCROLL_THRESHOLD` | 100 | 消息数超过此值启用虚拟滚动 |
| `overscan` | 5 | 可视区域外额外渲染的行数 |

**react-virtuoso 集成**：

```tsx
<Virtuoso
  followOutput="smooth"
  firstItemIndex={firstItemIndex}
  initialTopMostItemIndex={messages.length - 1}
  itemContent={(index) => <MessageBubble message={messages[index]} />}
  startReached={loadMoreMessages}
  isScrolling={(isScrolling) => { if (!isScrolling) checkAutoScroll() }}
/>
```

**与分页加载配合**：
- `startReached`：触发 `loadMoreMessages`（向上加载更早消息）
- `followOutput="smooth"`：新消息自动滚底
- `firstItemIndex`：分页加载时调整起始索引

### 3.3 暂缓理由

react-virtuoso 集成复杂（动态高度、跟随最新消息、分页加载配合），且当前对话很少超过 100 条。分页加载已解决大部分性能问题。

---

## 4. 客户端自动重试

### 4.1 设计

```typescript
interface RetryConfig {
  maxAttempts: number
  baseDelayMs: number
  maxDelayMs: number
}

const DEFAULT_RETRY_CONFIG: RetryConfig = {
  maxAttempts: 3,
  baseDelayMs: 1000,
  maxDelayMs: 30000,
}

async function retryWithBackoff(
  fn: () => Promise<void>,
  config: RetryConfig,
  isRetryable: (error: ChatError) => boolean
): Promise<void> {
  for (let attempt = 0; attempt < config.maxAttempts; attempt++) {
    try {
      await fn()
      return
    } catch (error) {
      if (!isRetryable(error as ChatError) || attempt === config.maxAttempts - 1) {
        throw error
      }
      const delay = Math.min(
        config.baseDelayMs * Math.pow(2, attempt) + Math.random() * 1000,
        config.maxDelayMs
      )
      await new Promise(resolve => setTimeout(resolve, delay))
    }
  }
}
```

**可重试错误**：`NetworkError` / `Timeout` / `RateLimited` / `ServerError` / `StreamError`

**不可重试错误**：`ApiKeyMissing` / `AuthFailed` / `ModelNotFound` / `ContextOverflow` / `Cancelled`

**退避策略**：`delay = min(baseDelayMs * 2^attempt + jitter, maxDelayMs)`

**UI 显示**：重试状态 "重试中 (2/3)..."

### 4.2 常量

```typescript
export const RETRY_MAX_ATTEMPTS = 3
export const RETRY_BASE_DELAY_MS = 1000
export const RETRY_MAX_DELAY_MS = 30000
```

### 4.3 与 I17 的关系

I17 的重试机制是**后端驱动**的（`'retry` 循环 + `RetryNotification` + `RetryBanner`），设计哲学是**用户主动选择恢复动作**。客户端自动重试与 I17 有以下关系：

| 维度 | I17 后端重试 | 客户端自动重试 |
|------|------------|--------------|
| 驱动方 | 后端 `'retry` 循环 | 前端 `retryWithBackoff` |
| 触发 | 后端自动（429/5xx） | 前端自动（网络错误/超时） |
| 用户控制 | RetryBanner 倒计时 + 取消 | UI 显示重试状态 |
| 适用场景 | HTTP 层错误（限流/服务端错误） | 请求层错误（网络/超时） |

**潜在互补**：后端重试耗尽后，前端可做二次自动重试（如网络恢复后）。但需注意与 I17 的 `retryFromError` 统一入口协调。

### 4.4 暂缓理由

流式场景下自动重试行为复杂（半截消息怎么重试？），当前流式出错用户手动重发即可。与 I17 的"用户主动选择恢复动作"设计哲学有张力。

---

## 5. 实施步骤

### Step 1：代码块语法高亮

1. `npm install shiki`
2. 新增 `CodeBlock.tsx`：shiki 渲染 + 语言标签 + 复制按钮
3. `MessageBubble.tsx`：ReactMarkdown components 集成 CodeBlock
4. CSS 样式：代码块容器 + 头部 + 行号

**验证**：代码块语法高亮正确，复制按钮可用，亮/暗主题切换。

### Step 2：虚拟滚动

1. `npm install react-virtuoso`
2. `MessageList.tsx`：消息数 > `VIRTUAL_SCROLL_THRESHOLD` 时使用 Virtuoso，否则使用普通滚动
3. 与分页加载配合：`startReached` / `firstItemIndex`
4. 与自动滚底配合：`followOutput="smooth"` + "回到底部"按钮

**验证**：200 条消息对话，DOM 节点数 < 60。

### Step 3：客户端自动重试

1. 新增 `src/chat/retryWithBackoff.ts`
2. `chatStore.ts`：`sendMessage` 中对可重试错误使用 `retryWithBackoff`
3. UI：重试状态显示

**验证**：模拟网络断开 → 恢复 → 消息自动重试成功。

---

## 6. 验收标准

- [ ] **代码高亮**：代码块语法高亮 + 语言标签 + 复制按钮
- [ ] **虚拟滚动**：200 条消息对话，DOM 节点数 < 60
- [ ] **虚拟滚动+分页**：虚拟滚动中向上滚动触发分页加载
- [ ] **自动重试**：可重试错误自动重试，不可重试错误不重试
- [ ] **退避策略**：重试间隔按指数退避递增
- [ ] **重试状态**：UI 显示 "重试中 (2/3)..."
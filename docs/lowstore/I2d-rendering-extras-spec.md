# I2d — 渲染增强规格说明

> **来源**：从 I2-History §4.10 / §4.11 / §6.3 提取
> **前置**：I2-History ✅ 已完成

## 1. 目标

提升消息渲染性能和体验：
- **代码块语法高亮**：集成 `shiki`，支持 100+ 语言 + 语言标签 + 复制按钮

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

---

## 3. 实施步骤

### Step 1：代码块语法高亮

1. `npm install shiki`
2. 新增 `CodeBlock.tsx`：shiki 渲染 + 语言标签 + 复制按钮
3. `MessageBubble.tsx`：ReactMarkdown components 集成 CodeBlock
4. CSS 样式：代码块容器 + 头部 + 行号

**验证**：代码块语法高亮正确，复制按钮可用，亮/暗主题切换。

---

## 4. 验收标准

- [ ] **代码高亮**：代码块语法高亮 + 语言标签 + 复制按钮
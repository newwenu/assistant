# Assistant

基于 Tauri 2 + React 19 的桌面 AI 助手：常驻桌面的 Orb 悬浮球承载多模型对话，主窗口提供工具箱、聊天历史、密码保险库与设置体系。

## 功能

- **Orb 悬浮球**：透明置顶、拖拽吸附、鼠标穿透、悬停隐藏，点击展开对话面板
- **AI 对话**：多 Provider 接入（OpenAI / Anthropic / Gemini 等），SSE 流式输出，支持 Agent 模式
- **上下文压缩**：接近窗口上限时自动生成交接摘要，对话锁定后可由用户继续
- **消息历史**：分页、搜索、置顶、分支切换、编辑/重试、Markdown/JSON 导出
- **工具箱**：分类浏览，支持搜索、收藏、最近使用与命令面板（Ctrl/Cmd+K），主窗口提供普通与紧凑两种布局
- **密码保险库**：本地加密存储，支持 TOTP、泄露检查、生物识别解锁、锁屏、恢复密钥与 Git 同步
- **Mascot**：Rive 动画形象，随对话状态联动表情
- **设置**：集中管理 Provider、Agent、存储、保险库等，跨窗口实时同步
- **国际化**：中文 / English

## 技术栈

| 层 | 技术 |
|---|---|
| 前端 | React 19、TypeScript、Vite、Ant Design、Zustand、React Router、i18next |
| 后端 | Rust、Tauri 2、rusqlite、tokio |
| 数据 | SQLite（业务）、SQLCipher（保险库）、JSON 配置（AppConf / OrbConf / MascotConf） |

## 许可

本项目采用 [MIT License](LICENSE)。

随应用分发的第三方资源各自保留原许可：Inter 与 JetBrains Mono 字体（SIL OFL 1.1）、SQLCipher（BSD-style，经 `rusqlite` 的 `bundled-sqlcipher` 链接）、Rive Runtime（MIT）、models.dev 模型清单（MIT）

# Assistant

基于 Tauri 2 + React 19 的桌面 AI 助手：常驻桌面的 Orb 悬浮球承载多模型对话，主窗口提供 22 个常用工具箱、聊天历史管理、密码保险库与完整设置体系。

## 功能

- **Orb 悬浮球**：透明置顶、拖拽吸附、鼠标穿透、悬停隐藏、点击展开对话面板
- **AI 对话**：OpenAI / OpenAI Responses / Anthropic / Gemini（含 Code Assist）多 Provider，SSE 流式输出，Agent 模式（内置 + 自定义），token 用量追踪
- **上下文压缩**：达到窗口 80% 自动生成交接摘要，对话锁定并提供“继续对话”
- **消息历史**：分页、搜索、置顶、分支切换、编辑/重试、Markdown/JSON 导出
- **工具箱**：dev / converter / security / media 四类共 22 个工具，支持搜索、收藏、最近使用与命令面板（Ctrl/Cmd+K）
- **密码保险库**：Argon2id + AES-256-GCM/ChaCha20-Poly1305 加密，SQLCipher 元数据库，TOTP、HIBP、生物识别、锁屏、恢复密钥、SSH Agent、Git 同步
- **Mascot**：Rive 动画形象，随对话状态联动表情
- **设置**：通用 / 外观 / 快捷键 / Provider / Agent / 存储 / 保险库 / 关于，跨窗口实时同步
- **国际化**：中文 / English

## 技术栈

| 层 | 技术 |
|---|---|
| 前端 | React 19、TypeScript、Vite、Ant Design、Zustand、React Router、i18next |
| 后端 | Rust、Tauri 2、rusqlite、tokio |
| 数据 | SQLite（业务）、SQLCipher（保险库）、JSON 配置（AppConf / OrbConf / MascotConf） |

## 开发

前置：Node.js 20+、Rust stable、Tauri 2 平台依赖（Windows 需 WebView2 与 MSVC）。

```bash
npm install
npm run tauri dev
```

常用脚本：

```bash
npm run typecheck   # tsc --noEmit
npm run lint        # eslint src
npm run test        # vitest run
npm run tauri build # 打包
```

Cargo 工作区位于 `src-tauri/`，根包为 `assistant-app`，成员 crate 见 `src-tauri/Cargo.toml`。

## 目录

```
src/            React 前端（orb / chat / tools / history / vault / settings / stores ...）
src-tauri/      Rust 后端（Tauri 集成层 + 9 个 workspace crate）
docs/           架构、设计、迭代规格与实施记录
references/     参考项目克隆（已 gitignore，不参与构建）
```

开发前建议先读 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)；各阶段规格与任务清单在 `docs/store/`，历史存档在 `docs/achievement*/`。

## 状态

核心功能（I0–I7、保险库、图片附件）已完成。尚未开始：三平台打包发布与自动更新（I8）、非图片文件附件、Vault 同步超时/取消、消息代码高亮、i18n 全覆盖、CI 与 Rust 测试补齐。详见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) §9“后续迭代”。

## 许可证

MIT

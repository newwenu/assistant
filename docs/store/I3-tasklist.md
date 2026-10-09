# I3 — 主窗口工具箱 任务清单

> 规格：[I3-spec.md](./I3-spec.md)
> 前置：I2 ✅ 已完成
> **原 I4（工具箱扩展）已合并入 I3**，一次性交付 20+ 工具

---

## 阶段 A — Rust 后端：工具历史 + 收藏 CRUD ✅

- [x] A1. `tool_history.rs`：ToolHistoryRow + record_usage + get_recent_tools + cleanup_old_history + lib.rs 导出
- [x] A2. `tool_favorites.rs`：get_favorites + toggle_favorite + set_favorites_order + lib.rs 导出
- [x] A3. Tauri Commands：record_tool_usage / get_recent_tools / get_favorite_tools / toggle_tool_favorite / set_favorite_tools_order（5 个命令注册到 invoke_handler）

---

## 阶段 B — 前端：类型 + 注册表 + Store + 搜索 ✅

- [x] B1. 前端依赖安装：fuse.js ^7.5.0 + @dnd-kit/core ^6.3.1 + @dnd-kit/sortable ^10.0.0 + @dnd-kit/utilities ^3.2.2 + date-fns ^4.4.0
- [x] B2. 工具类型定义：types.ts（ToolDefinition / ToolCategory / ToolCategoryMeta / WidthMode）+ categories.ts（8 分类：crypto/converter/web/dev/text/math/daily/media）+ defineTool 工厂
- [x] B3. 工具注册表：registry.ts 导入 12 个工具定义，导出 tools / toolsByCategory / toolsMap，React.lazy 懒加载（后续阶段 D5 新增 9 个工具时扩展注册表）
- [x] B4. toolStore：Zustand store — loadFavorites / toggleFavorite / reorderFavorites / loadRecentTools / recordUsage / search（fuse.js 实例）
- [x] B5. 搜索系统：ToolSearchBar.tsx + fuse.js（name:w2, keywords:w1.5, description:w1, threshold:0.3）+ 分类分组
- [x] B6. 命令面板：CommandPalette.tsx — Ctrl/Cmd+K 唤起 + 键盘导航 + 混合搜索（工具/操作/设置）+ 每分类 5 条

---

## 阶段 C — 前端：UI 组件 + 路由集成 ✅

- [x] C1. ToolBox 首页：搜索框 + 收藏拖拽区 + 最近使用区 + 新工具区 + 分类分组 + 空结果提示
- [x] C2. ToolCard 组件：图标(28px) + isNew 徽章 + 收藏按钮 + 名称 + 描述(line-clamp-2) + 点击导航
- [x] C3. FavoriteButton 组件：HeartFilled/HeartOutlined + toggleFavorite
- [x] C4. ToolPage 统一布局：标题(28px) + 收藏 + 分隔线 + 描述 + widthMode(max-width) + Suspense 懒加载
- [x] C5. CollapsibleToolMenu 侧边栏：收藏置顶 + 折叠/展开 + localStorage 持久化 + 当前高亮 + 工具数量
- [x] C6. 路由 + 侧边栏集成：App.tsx `/` → ToolBox, `/tools/:id` → ToolPage, 侧边栏工具分类菜单, recordUsage
- [x] C7. 常量新增：TOOL_DEFAULT_MAX_WIDTH / TOOL_WIDE_MAX_WIDTH / TOOL_CARD_GAP / TOOL_SEARCH_THRESHOLD / TOOL_NEW_WEEKS / RECENT_TOOLS_LIMIT / COMMAND_PALETTE_MAX_PER_CATEGORY

---

## 阶段 D — 工具实现（21 个） ✅

- [x] D1. 加密类（3）：hash-text（SHA1/SHA256/SHA512）+ uuid-generator（v1/v4+批量）+ base64-string-converter（编解码切换）
- [x] D2. 转换器类（4）：color-converter（HEX/RGB/HSL/RGBA+色板）+ date-time-converter（时间戳↔日期）+ integer-base-converter（2/8/10/16进制）+ url-encoder（编解码+URL解析）
- [x] D3. 开发类（3）：json-viewer（格式化/压缩/验证+排序键）+ regex-tester（匹配高亮+捕获组）+ jwt-parser（解码+过期检查）
- [x] D4. 文本类（2）：text-diff（双栏+统一/拆分视图）+ text-statistics（字符/单词/行/字节统计）
- [x] D5. 扩展工具（9）：
  - [x] D5a. 开发类（2）：sql-formatter ✅（格式化/压缩+多方言13种+关键字大小写+缩进风格+别名对齐tabulateAlias）+ cron-parser ✅（Cron表达式解析+中文locale(zh_CN)+人类可读描述+周日=0开关+@annually/@midnight/@reboot别名+下次5次执行时间+参考表）
  - [x] D5b. 日常类（2）+数学类（1）+转换器类（1）+网络类（1）：qrcode-generator ✅（文本/URL→二维码+前景色/背景色/容错等级+下载PNG+复制图片）+ password-generator ✅（可配置长度4-128/字符集开关/安全随机/强度评估/熵/破解耗时）+ calculator ✅→math（mathjs表达式求值+历史记录+Enter计算）+ unit-converter ✅→converter（7类别：长度/重量/温度8温标(°C/°F/K/°R/°De/°N/°Ré/°Rø)/面积/体积/速度/数据互转+卡片网格布局）+ ip-info ✅→web（IPv4/IPv6验证+十进制/十六进制/二进制/IPv6映射转换+ipinfo.io地理位置查询+默认IP 192.168.1.1）
  - [x] D5c. 多媒体类（2）：image-compressor ✅（上传+输出格式JPEG/PNG/WebP+质量1-100+最大宽高+Canvas压缩+原图/压缩对比+节省百分比+下载+ObjectURL内存回收）+ svg-preview ✅（SVG代码编辑+DOMParser验证+实时预览+优化去注释/压缩空白+SVG/PNG下载+ObjectURL内存回收）
  - [x] D5d. 注册表扩展：registry.ts 新增 sql-formatter + cron-parser + calculator(math) + unit-converter(converter) + ip-info(web) + image-compressor + svg-preview defineTool + React.lazy 懒加载（共 21 工具，8 分类均有工具）
  - [x] D5e. 分类扩展：categories.ts 新增 media 分类定义（PictureOutlined, order:7）+ types.ts ToolCategory 新增 "media"；分类调整：calculator→math / unit-converter→converter / ip-info→web（消除 math/web 空分类）

---

## 阶段 E — 集成验证 ✅

### E1. 编译验证 ✅

- [x] `cargo check` 通过（仅 2 个 unused_imports warning）
- [x] `tsc --noEmit` 通过
- [x] `npm run lint` 0 errors（12 warnings：no-explicit-any + exhaustive-deps，非阻塞）

**依赖**：D1~D4

---

### E2. 功能验证 ✅（12 工具）

- [x] 工具箱首页 12 个工具卡片可见，按分类分组 — ToolBox.tsx 渲染 toolsByCategory，registry.ts 注册 12 工具
- [x] 最近使用工具在首页「最近使用」区域展示 — ToolBox.tsx recentToolDefs 区块，toolStore.loadRecentTools 调用 Rust get_recent_tools
- [x] 搜索框实时过滤 — ToolSearchBar.tsx 输入触发 toolStore.search，fuse.js 实时搜索
- [x] 工具卡片点击→工具页渲染 — ToolCard onClick navigate(`/tools/${tool.id}`)，App.tsx `/tools/:id` → ToolPage
- [x] 收藏添加/移除/拖拽排序 — FavoriteButton toggleFavorite，ToolBox DndContext + SortableContext + reorderFavorites
- [x] 侧边栏折叠/展开持久化 — CollapsibleToolMenu localStorage COLLAPSED_KEY 持久化
- [x] 命令面板 Ctrl/Cmd+K 搜索 — CommandPalette.tsx keydown 监听 (metaKey/ctrlKey)+k，混合搜索 Tools/Actions/Settings
- [x] 工具使用历史记录 — ToolPage useEffect recordUsage，toolStore.recordUsage 调用 Rust record_tool_usage
- [x] 12 个工具功能可用 — 逐文件验证：hash-text/uuid-generator/base64/color-converter/date-time/integer-base/url-encoder/json-viewer/regex-tester/jwt-parser/text-diff/text-statistics 均有完整输入→处理→输出→复制实现

**依赖**：E1

---

### E3. 验收标准 ✅（12 工具阶段）

- [x] I3-spec §6 验收标准 1~11 项通过（12 工具阶段）：
  1. ✅ 主窗口工具箱首页可见 12 个工具卡片，按分类分组
  2. ✅ 搜索框输入关键词，实时过滤工具列表
  3. ✅ 点击工具卡片 → 跳转工具页 → 懒加载组件渲染
  4. ✅ 工具页标题→分隔线→描述→内容层级正确
  5. ✅ 收藏按钮可添加/移除，收藏工具在侧边栏和首页置顶
  6. ✅ 收藏工具可拖拽排序
  7. ✅ 最近使用工具在首页「最近使用」区域展示，数据来自 tool_history
  8. ✅ 侧边栏分类可折叠/展开，折叠状态持久化
  9. ✅ Ctrl/Cmd+K 唤起命令面板，搜索工具+操作+设置
  10. ✅ 工具使用后 `tool_history` 表写入记录
  11. ✅ `cargo check` ✅ / `tsc --noEmit` ✅ / `lint 0 errors` ✅

**依赖**：E2

---

### E4. 扩展工具集成验证 ✅

- [x] E4a. 工具箱首页 21 个工具卡片可见，8 分类均有工具（crypto:3 / converter:5 / dev:5 / text:2 / math:1 / web:1 / daily:2 / media:2）— registry.ts 注册 21 工具，categories.ts 含 8 分类
- [x] E4b. 9 个扩展工具功能可用（sql-formatter/cron-parser/qrcode-generator/calculator/unit-converter/password-generator/ip-info/image-compressor/svg-preview）— 逐文件验证均有完整输入→处理→输出→复制实现；ObjectURL 内存泄漏已修复；cronstrue 中文 locale 已启用
- [x] E4c. `tsc --noEmit` ✅ / 新文件 `lint 0 errors` ✅
- [x] E4d. I3-spec §6 验收标准第 12 项：21 工具全部功能可用（超过 20+ 要求）

**依赖**：D5
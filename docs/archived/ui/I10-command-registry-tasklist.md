# I10 — 命令面板注册表化 任务清单

> 规格：[I10-command-registry-spec.md](./I10-command-registry-spec.md)
> 前置：I8 ✅（声明式设置系统）、I3 ✅（工具箱注册表）

---

## 阶段 1 — 实现

- [x] 1.1 新建 `commandRegistry.tsx`：`CommandCategory` + `CommandDefinition`（`action: () => void | Promise<void>`）+ `COMMAND_CATEGORIES` + `defineCommand()` + `getAllCommands()` + `clearCommands()`
- [x] 1.2 新建 `commandAggregation.tsx`：`mergeKeywords()`（自动推导 ∪ 注册者 keywords）+ `aggregateCommands(navigate)`（先 clear 再注册）+ `registerSettingsCommands`（遍历 SETTINGS_PAGES，icon 按 categoryId 映射，分类标签从 SETTINGS_CATEGORIES 获取）+ `registerToolCommands`（遍历 tools，icon 从 ComponentType 渲染为 `<ToolIcon />`）+ `registerManualCommands`（toggle-theme / random-tool / github / bug-report / history，action 执行时通过 `useSettingsStore.getState()` 读最新状态）+ `registerSettingFieldCommands`（遍历 `getAllDefinitions()` 生成直达设置项命令，action 带 `?focus={key}` 查询参数）
- [x] 1.3 新建 `commandPaletteBindings.ts`：`useCommandPaletteShortcut(binding, onToggle)`，复用 `shortcutTypes.ts` 的 `IS_MAC`（已 export）
- [x] 1.4 重写 `CommandPalette.tsx`：Spotlight 风格浮层（替代 Modal）+ `aggregateCommands(navigate)` + Fuse.js 搜索 + 轻量分类标题（非 Divider）+ 选中态左侧 accent 条。迁移 CSS module，更新 App.tsx import 路径，删除旧文件
- [x] 1.5 键盘交互重写：↓ 从输入框跳到结果 / ↑ 从首项回输入框 / Tab 跨分类 / Ctrl/Cmd+N/P / Ctrl+Enter 保持面板 / 1-9 仅在 selectedIndex≥0 时执行 / 鼠标 hover 同步 selectedIndex（无独立 hoveredIndex）
- [x] 1.6 CSS 重写：Spotlight 浮层定位（fixed top 15vh center 560px）+ 选中态 accent 条 + 空状态 + 结果区 50vh + 瞬时开关（无动画）+ CSS 变量暗色适配（无硬编码 fallback）
- [x] 1.7 设置页 `?focus=` 参数消费：`SettingsGroup.tsx` 读取 `useSearchParams().get("focus")`，给 `Form.Item` 加 `id={`setting-${d.key}`}`，mount 时 `scrollIntoView({ behavior: "smooth" })` + 临时高亮动画
- [x] 1.8 架构清理：`shortcutTypes.ts` export `IS_MAC`/`PRIMARY_MOD`；删除 `settingsStore.ts` 重复的 `IS_MAC`/`PRIMARY_MOD`/`DEFAULT_KEYBINDINGS`（改为从 `shortcutTypes.ts` 导入）；`ShortcutRecorder.tsx` 的 `formatDisplay` 改为复用 `formatBinding`

---

## 阶段 2 — 测试与编译

- [x] 2.1 新建 `commandRegistry.test.ts`：`defineCommand` + `getAllCommands` + `clearCommands`
- [x] 2.2 新建 `commandAggregation.test.ts`：设置页命令数 = SETTINGS_PAGES.length、工具命令数 = tools.length、设置项命令数 = getAllDefinitions().length、手动命令数 ≥ 5、`mergeKeywords` 推导规则
- [x] 2.3 `tsc --noEmit` + `eslint --fix` 通过

---

## 阶段 3 — 验收

- [x] 3.1 Ctrl/Cmd+K 唤起命令面板
- [x] 3.2 搜索设置页/工具/操作/外部链接均可达（"外观"/"theme"/"颜色"/"github"）
- [x] 3.3 关键词自动推导生效（"settings"/"appearance" 可搜到外观设置）
- [x] 3.4 选择命令后正确执行 action，每分类最多显示 `COMMAND_PALETTE_MAX_PER_CATEGORY` 条
- [x] 3.5 搜索设置项 label 可直达具体设置项（"开机自启"/"launch"），页面自动滚动到目标字段并高亮
- [x] 3.6 面板为 Spotlight 浮层风格，分类用轻量标题，选中项左侧 accent 条
- [x] 3.7 无 `<mark>` 高亮，空状态有提示
- [x] 3.8 ↓ 从输入框跳到结果，↑ 从首项回输入框，Tab 跨分类
- [x] 3.9 Ctrl/Cmd+N/P 等效 ↓↑，Ctrl+Enter 保持面板，1-9 仅在选中态时快捷
- [x] 3.10 鼠标 hover 同步键盘锚点，↓/↑ 从 hover 位置继续
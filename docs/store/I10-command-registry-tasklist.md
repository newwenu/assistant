# I10 — 命令面板注册表化 任务清单

> 规格：[I10-command-registry-spec.md](./I10-command-registry-spec.md)
> 前置：I8 ✅（声明式设置系统）、I3 ✅（工具箱注册表）

---

## 阶段 1 — 实现

- [ ] 1.1 新建 `commandRegistry.tsx`：`CommandCategory` + `CommandDefinition`（`action: () => void | Promise<void>`）+ `COMMAND_CATEGORIES` + `defineCommand()` + `getAllCommands()` + `clearCommands()`
- [ ] 1.2 新建 `commandAggregation.tsx`：`mergeKeywords()`（自动推导 ∪ 注册者 keywords）+ `aggregateCommands(navigate)`（先 clear 再注册）+ `registerSettingsCommands`（遍历 SETTINGS_PAGES，icon 按 categoryId 映射，分类标签从 SETTINGS_CATEGORIES 获取）+ `registerToolCommands`（遍历 tools，icon 从 ComponentType 渲染为 `<ToolIcon />`）+ `registerManualCommands`（toggle-theme / random-tool / github / bug-report / history，action 执行时通过 `useSettingsStore.getState()` 读最新状态）
- [ ] 1.3 新建 `commandPaletteBindings.ts`：`useCommandPaletteShortcut(binding, onToggle)`，复用 `shortcutTypes.ts` 的 `IS_MAC`
- [ ] 1.4 重写 `CommandPalette.tsx`：纯 UI，`useMemo` 调用 `aggregateCommands(navigate)` + Fuse.js + 分组渲染 + 键盘导航，≤ 150 行。迁移 CSS module，更新 App.tsx import 路径，删除旧文件
- [ ] 1.5 架构清理：删除 `settingsStore.ts` 重复的 `IS_MAC`/`PRIMARY_MOD`（改为从 `shortcutTypes.ts` 导入），`ShortcutRecorder.tsx` 的 `formatDisplay` 改为复用 `formatBinding`

---

## 阶段 2 — 测试与编译

- [ ] 2.1 新建 `commandRegistry.test.ts`：`defineCommand` + `getAllCommands` + `clearCommands`
- [ ] 2.2 新建 `commandAggregation.test.ts`：设置页命令数 = SETTINGS_PAGES.length、工具命令数 = tools.length、手动命令数 ≥ 5、`mergeKeywords` 推导规则
- [ ] 2.3 `tsc --noEmit` + `eslint --fix` + `cargo check` 通过

---

## 阶段 3 — 验收

- [ ] 3.1 Ctrl/Cmd+K 唤起命令面板
- [ ] 3.2 搜索设置页/工具/操作/外部链接均可达（"外观"/"theme"/"颜色"/"github"）
- [ ] 3.3 关键词自动推导生效（"settings"/"appearance" 可搜到外观设置）
- [ ] 3.4 选择命令后正确执行 action，每分类最多显示 `COMMAND_PALETTE_MAX_PER_CATEGORY` 条

---

## P2 增强（后续迭代）

- [ ] `commandAggregation.tsx` 新增 `registerSettingFieldCommands`：遍历 `getAllDefinitions()` 生成直达设置项命令（`?focus={key}`）
- [ ] 设置页支持 `?focus={key}` URL 参数，自动滚动高亮
- [ ] 验收：搜索 "开机自启" 可直达设置项
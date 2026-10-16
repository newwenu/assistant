# I11 — 国际化（i18n）任务清单

> 规格：[I11-i18n-spec.md](./I11-i18n-spec.md)
> 硬编码清单：[I11-A-hardcoded-strings.md](./I11-A-hardcoded-strings.md)
> 前置：I8 ✅（声明式设置系统）

---

## 阶段 A — 基础设施

- [x] A1. 引入 `i18next` + `react-i18next` + `i18next-browser-languagedetector` 依赖
- [x] A2. 创建 `src/i18n/` 目录，含初始化入口 `index.ts`
- [x] A3. 创建 `src/i18n/locales/zh-CN.json` + `en-US.json`，初始 key 集合
- [x] A4. 应用入口 `import "@/i18n"` 确保早于组件渲染
- [x] A5. `fallbackLng` 设为 `zh-CN`
- [x] A6. 导出 `translateKey()` 函数，接受 `string` 类型 key（绕过 i18next 严格类型签名）
- [x] A7. `missingKeyHandler` 配置，开发期 console.warn 缺失 key

---

## 阶段 B — 语言切换链路

- [x] B1. `localeStore.ts` — zustand store 管理 `language` 状态
- [x] B2. `main.tsx` — `applyLocaleChange()` 函数：`i18n.changeLanguage()` + `useLocaleStore.setLocale()`
- [x] B3. `settingEvents.ts` — 监听 `AppConfChanged`，`language in patch` 时调用 `applyLocaleChange`
- [x] B4. `initSettingSystem.ts` — 启动时读取 `AppConf.language` 并应用
- [x] B5. Ant Design `ConfigProvider.locale` 联动 — `ANTD_LOCALE_MAP` + 动态绑定
- [x] B6. date-fns locale 联动 — `formatDistanceToNow` 等函数传入动态 locale 参数

---

## 阶段 C — P0：设置系统定义层

- [x] C1. `pageRegistry.ts` — 页面 `label` 改为 i18n key
- [x] C2. `pageRegistry.ts` — 分类 `label` 改为 i18n key
- [x] C3. `fieldDefinitions.ts` — 所有 `label` / `extra` / `saveMessage` / `options.label` / `description` / `placeholder` 改为 i18n key
- [x] C4. `SettingsGroup.tsx` — 翻译边界，统一调用 `translateKey()` 翻译 schema 中所有 i18n key
- [x] C5. `SettingField.tsx` — 纯渲染，接收已翻译的 props，不 import `useTranslation`
- [x] C6. zh-CN.json / en-US.json — `settings` 域所有 key + 翻译

---

## 阶段 D — P1：Chat 错误体系

- [x] D1. `chatErrors.ts` — `getErrorDisplay()` 使用 `translateKey()` 翻译错误标题
- [x] D2. `chatErrors.ts` — `formatRetryAfter()` 使用 `translateKey()` + 插值
- [x] D3. zh-CN.json / en-US.json — `chat.error` 域所有 key + 翻译

---

## 阶段 E — P2：工具箱注册表 + 分类

- [x] E1. `registry.ts` — 21 个工具的 `name` / `description` 改为 i18n key
- [x] E2. `categories.ts` — 9 个分类 `label` 改为 i18n key
- [x] E3. `ToolCard.tsx` — 渲染层加 `translateKey()`，翻译 name / description / "新" tag
- [x] E4. `ToolBox.tsx` — 翻译 "收藏" / "最近使用" / "新工具" / "未找到匹配" / 分类 label / 工具名
- [x] E5. `ToolSearchBar.tsx` — placeholder 翻译
- [x] E6. `CollapsibleToolMenu.tsx` — 翻译 "收藏" / "工具箱" / 分类 label / 工具名
- [x] E7. `ToolPage.tsx` — 翻译 "组件加载失败" / "重新加载" / "工具未找到" / 工具名 / 描述
- [x] E8. `toolStore.ts` — Fuse.js 搜索索引使用翻译后文本，语言切换时重建索引
- [x] E9. zh-CN.json / en-US.json — `tools` 域所有 key + 翻译（分类名 + UI 文案 + 21 个工具名/描述）

---

## 阶段 F — P3：设置共享组件

- [x] F1. `ThemeRadioRender.tsx` — 使用 `useTranslation`
- [x] F2. `ShortcutRecorder.tsx` — placeholder / "未设置" 翻译
- [x] F3. `SettingsGroup.tsx` — "已保存" 翻译
- [x] F4. `MascotColorGroupRender.tsx` — 所有文案翻译
- [x] F5. `KeybindingField.tsx` — 冲突提示 / "已修改" 翻译
- [x] F6. `AssetPickerField.tsx` — 所有文案翻译

---

## 阶段 G — P4：Provider 管理对话框

- [x] G1. `ProviderManager.tsx` — 删除确认 / 标题 / 按钮文案翻译
- [x] G2. `ConnectDialog.tsx` — 表单 label / placeholder / 按钮翻译
- [x] G3. `AddProviderDialog.tsx` — 标题 / 选项翻译
- [x] G4. `ShortcutSettings.tsx` — "恢复默认" / 搜索 placeholder 翻译
- [x] G5. `AgentModeSettings.tsx` — 所有文案翻译
- [x] G6. `AboutSettings.tsx` — 字段名翻译

---

## 阶段 H — P5：命令面板 + 其他组件

- [x] H1. `commandRegistry.tsx` — `COMMAND_CATEGORIES` label 改为 i18n key（非模块级翻译）
- [x] H2. `commandAggregation.tsx` — `registerSettingsCommands` / `registerSettingSectionCommands` / `registerSettingFieldCommands` 中 name / description / keywords 使用 `translateKey()` 翻译
- [x] H3. `commandAggregation.tsx` — `registerToolCommands` 中 name / description / keywords 使用 `translateKey()` 翻译
- [x] H4. `commandAggregation.tsx` — `registerManualCommands` 中所有文案使用 `translateKey()` 翻译
- [x] H5. `CommandPalette.tsx` — `useMemo` 依赖加 `currentI18n.language`，语言切换时重新注册命令
- [x] H6. `CommandPalette.tsx` — `getAllCommands()` 展开为新数组 `[...getAllCommands()]`，打破可变引用相等性陷阱
- [x] H7. `CommandPalette.tsx` — 分类标题渲染时 `translateKey(meta.label)`
- [x] H8. `CommandPalette.tsx` — placeholder / "没有匹配" 使用 `t()` 翻译
- [x] H9. `HistoryManager.tsx` — 所有文案翻译（相对时间改用 date-fns locale）
- [x] H10. `ChatView.tsx` / `MessageInput.tsx` / `MessageActions.tsx` / `MessageList.tsx` — 对话区文案翻译
- [x] H11. 选择器组件 — `ProviderSelector` / `ModeSelector` / `ModelSelector` 翻译
- [x] H12. 对话块组件 — `ReasoningBlock` / `ErrorBlock` / `EditBlock` 翻译
- [x] H13. 通用组件 — `WindowControls` / `ThemeToggle` / `SettingsEntry` / `PinToggle` 翻译
- [x] H14. `OrbPanel.tsx` — 悬浮球面板文案翻译
- [ ] H15. 工具组件内部 — 21 个工具的内部 UI 文案翻译（按工具逐个处理）

---

## 阶段 I — P6：Rust 端

- [ ] I1. `assistant-i18n` crate 升级为 `rust_i18n` 驱动
- [ ] I2. Rust locale JSON 扩展至与前端同构的领域结构
- [ ] I3. `tray.rs` — 菜单项文本改为 `assistant_i18n::text()` 调用
- [ ] I4. `window.rs` — Orb 右键菜单文本改为 `assistant_i18n::text()` 调用
- [ ] I5. `cmd/conversation.rs` + `cmd/chat.rs` — 错误消息翻译
- [ ] I6. `agent_modes.rs` — 内置模式 seed 改为稳定 key，显示层翻译

---

## 阶段 J — 校验与质量保障

- [ ] J1. Key 一致性校验脚本 — 递归展开 JSON 后比对 zh-CN / en-US key 集合
- [ ] J2. 插值占位符一致性校验
- [ ] J3. ESLint 规则或 CI 脚本检测残留中/英文字面量
- [ ] J4. 豁免区域白名单配置（路由 path / CSS class / 技术标识）
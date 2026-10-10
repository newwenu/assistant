# I5-Ext — Mascot 可扩展渲染架构 任务清单

> 规格：[I5-extensible-renderer-spec.md](./I5-extensible-renderer-spec.md)
> 前置：I5 ✅（MascotFace 10 种 + FACE_TO_POSE + RiveMascot + mascotStore + useMascotChatSync）
> 实施日期：2026-10-10

---

## A — 编译基线

**提供**：编译通过、I5 残留已清理

- [x] A1. `tsc --noEmit` 通过；如有破损先修复
- [x] A2. 确认 `stateEngine` 引用已不存在（I5 阶段已清理）

---

## B — Core 层 → spec §4.1

**需要**：A
**提供**：`core/types`（MascotFace, MascotRendererId, RendererMeta, MascotRendererProps）、`core/resolve`（resolveVisemeCode, pickIdleFlourish）、`core/registry`（registerRenderer, getRenderer, getAllRendererMetas）

- [x] B1. 新建 `src/mascot/core/types.ts`
- [x] B2. 新建 `src/mascot/core/resolve.ts`
- [x] B3. 新建 `src/mascot/core/registry.ts`
- [x] B4. 新建 `src/mascot/core/index.ts`（re-export）
- [x] B5. 新建 `src/mascot/core/colorUtils.ts`（hexToArgb，跨渲染器通用颜色工具）

---

## C — 渲染器层 → spec §4.2

**需要**：B（core/types + core/resolve + core/registry）
**提供**：三个渲染器完整实现 + 自注册

### C1 — Rive 迁移

- [x] C1.1 新建 `src/mascot/renderers/rive/` 目录
- [x] C1.2 新建 `renderers/rive/riveStateEngine.ts`
- [x] C1.3 迁移 `riveMaps.ts` → `renderers/rive/riveMaps.ts`，移除被 Core 替代的导出（FACE_TO_POSE / faceToPose / AMBIENT_POSES / pickAmbientPose）
- [x] C1.4 迁移 `RiveMascot.tsx` → `renderers/rive/RiveMascot.tsx`，更新 import 路径
- [x] C1.5 RiveMascot 核心逻辑改造（依赖替换 + 状态引擎参数化 + rendererConfig 提取）
- [x] C1.6 RiveMascot 新增 `stateEngine` 可选 prop（默认 BUILTIN_RIVE_STATE_ENGINE）
- [x] C1.7 新建 `renderers/rive/index.ts`，`registerRenderer()` 自注册

### C2 — SVG 渲染器

- [x] C2.1 新建 `renderers/svg/GhostyMascot.tsx`（纯 SVG + setTimeout 驱动 idle 循环 + FACE_PRESETS 10 种表情 + resolveVisemeCode + pickIdleFlourish）
- [x] C2.2 新建 `renderers/svg/index.ts`，`registerRenderer()` 自注册

### C3 — GIF 渲染器

- [x] C3.1 新建 `renderers/gif/GifMascot.tsx`（img + 圆形裁剪 + freezeGif 冻结首帧 + idle 冻结/非 idle 播放）
- [x] C3.2 新建 `renderers/gif/index.ts`，`registerRenderer()` 自注册

---

## D — 编排层 → spec §4.3

**需要**：B（core/types）、C（三个渲染器自注册完成）
**提供**：MascotRenderer 调度组件、mascotStore 扩展字段

### D1 — MascotRenderer

- [x] D1.1 新建 `src/mascot/MascotRenderer.tsx`（副作用 import + 注册表查找 + 优雅降级）
- [x] D1.2 Props 定义 + 能力自适应（`capabilities.visemeSync` + `capabilities.colorCustomization`）
- [x] D1.3 `useEffect` 追踪 renderer 变化，切换时调用旧渲染器 `dispose()`；WebGL 渲染器使用 `useCanvasSizeWatchdog`（rAF 轮询守卫）校正 canvas 尺寸，不依赖 `useOrbStore` / DOM 事件监听

### D2 — mascotStore

- [x] D2.1 import 路径更新（`MascotFace` / `MascotRendererId` 从 `./core/types`）
- [x] D2.2 确认不新增 `renderer` 字段（已在 `mascotConf` 中），不持有渲染器内部状态，不持有渲染器专属配置

---

## E — 接入层 → spec §3.1 + §6

**需要**：D（MascotRenderer + mascotStore.renderer）、C（RiveMascot 已迁移）
**提供**：消费端全部使用 MascotRenderer、旧文件清理、公共 API

### E1 — 消费端改造

- [x] E1.1 `OrbBall.tsx`：`RiveMascot` → `MascotRenderer`，颜色经 `hexToArgb()` 转换后传 `primaryColor` / `secondaryColor` 顶层 prop，渲染器专属配置经 `buildRendererConfig()` 映射
- [x] E1.2 `MessageInput.tsx`：同上
- [x] E1.3 全局搜索其余 `from "*/RiveMascot"` 引用，替换

### E2 — 旧文件清理

- [x] E2.1 删除 `src/mascot/faceStates.ts`
- [x] E2.2 删除 `src/mascot/riveMaps.ts`
- [x] E2.3 删除 `src/mascot/RiveMascot.tsx`
- [x] E2.4 删除 `src/settings/MascotColorSection.tsx`（被 MascotSection 统一替代）
- [x] E2.5 全局搜索旧 import 路径，更新

### E3 — 公共 API

- [x] E3.1 新建 `src/mascot/index.ts`（re-export 公共 API）

---

## F — 设置集成 → spec §5

**需要**：B（core/registry 提供 getAllRendererMetas / getRenderer）、D（mascotStore.renderer）
**提供**：MascotSection 设置 UI、Rust MascotConf 扩展

- [x] F1. 新建 `src/settings/MascotSection.tsx`（渲染方式 Select 从注册表动态生成，局限性提示，颜色区按 capabilities.colorCustomization 条件显隐 + dirty/应用/重置交互，渲染器专属设置按 renderer 条件渲染）
- [x] F2. `AppearanceSettings.tsx` 移除独立"吉祥物颜色"区，新增"助手形象" SettingsSection 嵌入 MascotSection
- [x] F3. Rust：`MascotConf` 新增 `renderer` + `rive_idle_pose_rotation` + `svg_animated` + `gif_src` 扁平强类型字段，`validate` 校验全字段（renderer 枚举 + hex 格式 + gif_src 非空）；TS `MascotConf` 镜像同步 4 个新字段；dispatcher `buildRendererConfig()` + `buildMascotPatch()` 映射
- [ ] F4. Rust：`cmd.rs` 新增 `install_mascot_asset` 通用命令（文件上传 + 大小限制 2MB + MIME 嗅探）
- [ ] F5. Rust：`lib.rs` invoke_handler 注册新命令

---

## G — 验证 → spec §11

**需要**：E + F（全部实现完成）
**提供**：编译通过 + 功能验证 + 验收通过

### G1 — 编译

- [x] G1.1 `tsc --noEmit` 通过
- [x] G1.2 `eslint` 0 errors
- [x] G1.3 `cargo check` 通过

### G2 — 功能

- [x] G2.1 Rive 回归：行为与改造前完全一致
- [x] G2.2 SVG：GhostyMascot 正常显示，表情随 face 变化
- [x] G2.3 GIF：播放/冻结二态正确（idle 冻结，非 idle 播放）
- [x] G2.4 Disabled：无 Mascot 渲染
- [x] G2.5 设置 UI：渲染方式切换立即生效
- [x] G2.6 `useMascotChatSync` 表情联动不受渲染器影响
- [x] G2.7 默认颜色：null → 内嵌色，选色 → 生效，重置 → null
- [x] G2.8 颜色区显隐：SVG/GIF 隐藏，Rive 显示

### G3 — 验收

- [x] G3.1 spec §11 验收标准 1~23 项全部通过

---

## 依赖视图

```
A
│
B
│
├──→ C ──→ D ──→ E ──┐
│              │       │
│         D2 ←─┘       │
│                      │
└──→ F ←───────────────┘
     │
     └──→ G
```

D 内部：D1 需要 C，D2 仅需 B，可并行
E 与 F：E 依赖 D，F 依赖 B+D2，可部分并行（F 在 D2 完成后即可开始）

---

## 实施备注

| 项目 | 说明 |
|------|------|
| 新增 B5 | `core/colorUtils.ts`（hexToArgb）是跨渲染器通用工具，消费端 OrbBall/MessageInput 也需要，规格未单独列出但实现中合理新增 |
| 新增 E2.4 | 删除 `MascotColorSection.tsx`，颜色设置统一在 MascotSection 内按 capabilities 条件显隐，消除重复 |
| F4/F5 未完成 | `install_mascot_asset` 通用文件上传命令未实现，属于后续迭代内容（当前 GIF src 通过手动配置 mascot-config.json 设置） |
| SVG 用 setTimeout | 规格提到 `useMascotClock` RAF 驱动，实际 SVG idle 循环用 setTimeout + pickIdleFlourish，无逐帧动画需求，更省 CPU |
| 颜色交互 dirty+应用 | 与原 MascotColorSection 保持一致的"修改→应用"模式，避免即时生效误操作 |
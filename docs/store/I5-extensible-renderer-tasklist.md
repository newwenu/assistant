# I5-Ext — Mascot 可扩展渲染架构 任务清单

> 规格：[I5-extensible-renderer-spec.md](./I5-extensible-renderer-spec.md)
> 前置：I5 ✅（MascotFace 10 种 + FACE_TO_POSE + RiveMascot + mascotStore + useMascotChatSync）

---

## 阶段 A — 修复破损代码（Step 0）

当前 `mascotStore.ts` 和 `RiveMascot.tsx` 引用了已删除的 `stateEngine.ts`，需先回滚恢复编译通过。

- [ ] A1. `mascotStore.ts` 移除对 `./stateEngine` 的 import，回滚 `MascotStateEngine` / `BUILTIN_STATE_ENGINE` / `MascotRendererId` 相关字段和方法，恢复为 I5 完成时的状态
- [ ] A2. `RiveMascot.tsx` 积除对 `./stateEngine` 的 import，回滚 `resolveFaceToPose` / `resolveVisemeCode` / `pickIdleFlourish` / `MascotStateEngine` 相关代码，恢复 `faceToPose` / `pickAmbientPose` / `toRiveVisemeCode` 调用
- [ ] A3. `faceStates.ts` 积除 `MascotRendererId` 类型导出（恢复为仅 `MascotFace`）
- [ ] A4. `tsc --noEmit` 通过，确认编译恢复

---

## 阶段 B — Protocol 层（Step 1）

- [ ] B1. 新建 `src/mascot/protocol/types.ts`：从 `faceStates.ts` 提取 `MascotFace` 类型 + 新增 `MascotRendererId` 类型
- [ ] B2. 新建 `src/mascot/protocol/stateEngine.ts`：`MascotChannel` + `MascotStateEngine` 接口定义
- [ ] B3. 新建 `src/mascot/protocol/index.ts`：re-export types + stateEngine

---

## 阶段 C — Resolution 层（Step 2）

- [ ] C1. 新建 `src/mascot/resolution/resolveFaceToPose.ts`：两级回退纯函数（engine.states[face] → engine.states.idle）
- [ ] C2. 新建 `src/mascot/resolution/resolveVisemeCode.ts`：viseme 码校验纯函数（精确匹配 → sil → 首项）
- [ ] C3. 新建 `src/mascot/resolution/pickIdleFlourish.ts`：空闲轮转 pose 选取纯函数
- [ ] C4. 新建 `src/mascot/resolution/index.ts`：re-export 三个函数

---

## 阶段 D — Built-in Config 层（Step 3）

- [ ] D1. 新建 `src/mascot/builtin/riveStateEngine.ts`：`BUILTIN_STATE_ENGINE` 常量（visemeCodes + states + idlePoseCycle），数据与原 `FACE_TO_POSE` + `RIVE_VISEME_SET` + `AMBIENT_POSES` 语义一致
- [ ] D2. 新建 `src/mascot/builtin/index.ts`：re-export

---

## 阶段 E — Renderers 层：Rive 迁移（Step 4）

- [ ] E1. 新建 `src/mascot/renderers/rive/` 目录
- [ ] E2. 迁移 `riveMaps.ts` → `renderers/rive/riveMaps.ts`：保留 `MASCOT_STATE_MACHINE` / `RIVE_POSES` / `RivePose` / `toRiveVisemeCode` / `pickVisemeCode` / `AMBIENT_POSES` / `pickAmbientPose`；移除 `FACE_TO_POSE` / `faceToPose`；import 路径从 `./faceStates` 改为 `../../protocol/types`
- [ ] E3. 迁移 `RiveMascot.tsx` → `renderers/rive/RiveMascot.tsx`：import 路径更新（`../../protocol/types` / `../../resolution` / `../../builtin` / `./riveMaps` / `../../../stores/orbStore`）
- [ ] E4. `renderers/rive/RiveMascot.tsx` 改造核心逻辑：`faceToPose(face)` → `resolveFaceToPose(face, engine)`；`toRiveVisemeCode(visemeCode)` → `resolveVisemeCode(toRiveVisemeCode(visemeCode), engine)`；`pickAmbientPose(...)` → `pickIdleFlourish(engine, ...)`
- [ ] E5. `renderers/rive/RiveMascot.tsx` 新增 `stateEngine?: MascotStateEngine` prop，默认 `BUILTIN_STATE_ENGINE`
- [ ] E6. 新建 `src/mascot/renderers/rive/index.ts`：re-export `RiveMascot`

---

## 阶段 F — Renderers 层：SVG + GIF（Step 6-7）

- [ ] F1. 新建 `src/mascot/renderers/svg/GhostyMascot.tsx`：纯 SVG 渲染组件，接受 `MascotFace` + `visemeCode` + `animated` + `stateEngine` props，用 SVG path 变换实现 bob/blink/wave/viseme（参考 OpenHuman Ghosty.tsx）
- [ ] F2. 新建 `src/mascot/renderers/svg/index.ts`：re-export `GhostyMascot`
- [ ] F3. 新建 `src/mascot/renderers/gif/GifMascot.tsx`：最简渲染器，`<img>` + `data-face`，接受 `src` + `face` + `size` props（参考 OpenHuman CustomGifMascot.tsx）
- [ ] F4. 新建 `src/mascot/renderers/gif/index.ts`：re-export `GifMascot`

---

## 阶段 G — Orchestration 层：MascotRenderer（Step 8）

- [ ] G1. 新建 `src/mascot/MascotRenderer.tsx`：渲染器调度组件，switch `renderer` 切换 RiveMascot / GhostyMascot / GifMascot / null
- [ ] G2. Props 定义：`renderer` / `face` / `visemeCode` / `size` / `idlePoseRotation` / `stateEngine` / `primaryColor` / `secondaryColor` / `customGifSrc`

---

## 阶段 H — mascotStore 改造（Step 9）

- [ ] H1. `mascotStore.ts` import 路径更新：`MascotFace` / `MascotRendererId` 从 `./protocol/types`，`MascotStateEngine` 从 `./protocol/stateEngine`，`BUILTIN_STATE_ENGINE` 从 `./builtin`
- [ ] H2. `MascotState` 新增 `renderer: MascotRendererId`（默认 `"rive"`）+ `stateEngine: MascotStateEngine`（默认 `BUILTIN_STATE_ENGINE`）+ `customGifSrc: string | null`（默认 `null`）
- [ ] H3. 新增 `setRenderer` / `setStateEngine` / `setCustomGifSrc` 方法

---

## 阶段 I — 消费端改造（Step 10）

- [ ] I1. `OrbBall.tsx`：`import { RiveMascot }` → `import { MascotRenderer }`；从 mascotStore 读取 `renderer` / `stateEngine`；`<RiveMascot ...>` → `<MascotRenderer renderer={renderer} stateEngine={stateEngine} ...>`
- [ ] I2. `MessageInput.tsx`：同上改造
- [ ] I3. 全局搜索所有 `from "../mascot/RiveMascot"` / `from "../../mascot/RiveMascot"` 引用，替换为 `MascotRenderer`

---

## 阶段 J — 旧文件清理（Step 12）

- [ ] J1. 删除 `src/mascot/faceStates.ts`（已被 `protocol/types.ts` 替代）
- [ ] J2. 删除 `src/mascot/riveMaps.ts`（已迁移至 `renderers/rive/riveMaps.ts`）
- [ ] J3. 删除 `src/mascot/RiveMascot.tsx`（已迁移至 `renderers/rive/RiveMascot.tsx`）
- [ ] J4. 全局搜索所有对旧文件的 import，更新路径

---

## 阶段 K — 公共 API + index.ts

- [ ] K1. 新建 `src/mascot/index.ts`：re-export `MascotRenderer` / `useMascotStore` / `useMascotChatSync` / `MascotFace` / `MascotRendererId` / `MascotStateEngine` / `BUILTIN_STATE_ENGINE`

---

## 阶段 L — 外观设置集成

- [ ] L1. 新建 `src/settings/MascotSection.tsx`：渲染方式 Select（Rive/SVG/GIF/禁用）+ 条件设置区（Rive: 主色调/辅色调/空闲动画；GIF: 图片路径）
- [ ] L2. `AppearanceSettings.tsx` 新增"助手形象" SettingsSection，嵌入 MascotSection
- [ ] L3. `settingsStore.ts` 新增 `MascotConf` 接口 + `mascotConf` 字段 + `loadMascotConf` / `saveMascotConf` 方法
- [ ] L4. Rust 后端：`assistant-config` 新增 `MascotConf` 结构体 + load/save
- [ ] L5. Rust 后端：`cmd.rs` 新增 `get_mascot_conf` / `save_mascot_conf` 命令
- [ ] L6. Rust 后端：`lib.rs` invoke_handler 注册新命令

---

## 阶段 M — 编译验证

- [ ] M1. `tsc --noEmit` 通过
- [ ] M2. `npm run lint` 0 errors
- [ ] M3. `cargo check` 通过

**依赖**：A1~A4, B1~B3, C1~C4, D1~D2, E1~E6, F1~F4, G1~G2, H1~H3, I1~I3, J1~J4, K1, L1~L6

---

## 阶段 N — 功能验证

- [ ] N1. `renderer === "rive"` 时行为与改造前完全一致（回归测试）
- [ ] N2. `renderer === "svg"` 时 GhostyMascot 正常显示，表情随 face 变化
- [ ] N3. `renderer === "gif"` 时 GifMascot 正常显示 GIF 动画
- [ ] N4. `renderer === "disabled"` 时无 Mascot 渲染
- [ ] N5. 外观设置"助手形象"区可切换渲染方式，立即生效
- [ ] N6. `useMascotChatSync` 表情联动正常（不受渲染器切换影响）

**依赖**：M1~M3

---

## 阶段 O — 验收标准

- [ ] O1. I5-extensible-renderer-spec.md §11 验收标准 1~15 项全部通过

**依赖**：N1~N6
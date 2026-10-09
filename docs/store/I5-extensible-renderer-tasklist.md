# I5-Ext — Mascot 可扩展渲染架构 任务清单

> 规格：[I5-extensible-renderer-spec.md](./I5-extensible-renderer-spec.md)
> 前置：I5 ✅（MascotFace 10 种 + FACE_TO_POSE + RiveMascot + mascotStore + useMascotChatSync）

---

## 阶段 A — 确认编译基线（Step 0）

> 当前代码中 `stateEngine` 引用已不存在，A 阶段可能已完成。先验证编译状态。

- [ ] A1. `tsc --noEmit` 通过，确认当前编译无破损
- [ ] A2. 如有破损，修复后再验证；如已通过，标记 A 阶段完成

---

## 阶段 B — Core 层（Step 1）

> 跨引擎共享的类型 + 通用工具函数

- [ ] B1. 新建 `src/mascot/core/types.ts`：从 `faceStates.ts` 提取 `MascotFace` 类型 + 新增 `MascotRendererId` 类型
- [ ] B2. 新建 `src/mascot/core/resolve.ts`：`resolveVisemeCode(code, visemeCodes: string[])` + `pickIdleFlourish(cycle: string[], exclude?, rng?)`
- [ ] B3. 新建 `src/mascot/core/index.ts`：re-export types + resolve

---

## 阶段 C — Renderers 层：Rive 迁移（Step 2）

- [ ] C1. 新建 `src/mascot/renderers/rive/` 目录
- [ ] C2. 新建 `renderers/rive/riveStateEngine.ts`：`RiveStateEngine` 接口（visemeCodes + states + idlePoseCycle + channels）+ `resolveFaceToPose` 函数（两级回退）+ `BUILTIN_RIVE_STATE_ENGINE` 常量（数据与原 `FACE_TO_POSE` + `RIVE_VISEME_SET` + `AMBIENT_POSES` 语义一致）
- [ ] C3. 迁移 `riveMaps.ts` → `renderers/rive/riveMaps.ts`：保留 `MASCOT_STATE_MACHINE` / `RIVE_POSES` / `RivePose` / `toRiveVisemeCode` / `pickVisemeCode`；移除 `FACE_TO_POSE` / `faceToPose` / `AMBIENT_POSES` / `pickAmbientPose`；import 路径从 `./faceStates` 改为 `../../core/types`
- [ ] C4. 迁移 `RiveMascot.tsx` → `renderers/rive/RiveMascot.tsx`：import 路径更新（`../../core` / `./riveStateEngine` / `./riveMaps` / `../../../stores/orbStore`）
- [ ] C5. `renderers/rive/RiveMascot.tsx` 改造核心逻辑：`faceToPose(face)` → `resolveFaceToPose(face, engine)`；`toRiveVisemeCode(visemeCode)` → `resolveVisemeCode(toRiveVisemeCode(visemeCode), engine.visemeCodes)`；`pickAmbientPose(...)` → `pickIdleFlourish(engine.idlePoseCycle, ...)`
- [ ] C6. `renderers/rive/RiveMascot.tsx` 新增 `stateEngine?: RiveStateEngine` prop，默认 `BUILTIN_RIVE_STATE_ENGINE`，组件内 `const engine = stateEngine ?? BUILTIN_RIVE_STATE_ENGINE`
- [ ] C7. 新建 `src/mascot/renderers/rive/index.ts`：re-export `RiveMascot`

---

## 阶段 D — Renderers 层：SVG + GIF（Step 3）

- [ ] D1. 新建 `src/mascot/renderers/svg/GhostyMascot.tsx`：纯 SVG 渲染组件，接受 `MascotFace` + `visemeCode` + `animated` props；组件内部定义 `FACE_PRESETS` / `SVG_VISEME_CODES` / `SVG_IDLE_CYCLE`；SVG path 变换实现 bob/blink/wave/viseme（参考 OpenHuman Ghosty.tsx）
- [ ] D2. 新建 `src/mascot/renderers/svg/index.ts`：re-export `GhostyMascot`
- [ ] D3. 新建 `src/mascot/renderers/gif/GifMascot.tsx`：GIF 渲染器，`<img>` + `data-face` + 圆形裁剪，接受 `src` + `face` + `size` + `playing` props；实现 `freezeGif()` 冻结技术（`canvas.drawImage → toDataURL`，参考 deepseek-harness-orb `floating.js`）；`playing=true` 时播放，`playing=false` 时冻结到首帧
- [ ] D4. 新建 `src/mascot/renderers/gif/index.ts`：re-export `GifMascot`

---

## 阶段 E — Orchestration 层：MascotRenderer（Step 4）

- [ ] E1. 新建 `src/mascot/MascotRenderer.tsx`：渲染器调度组件，switch `renderer` 切换 RiveMascot / GhostyMascot / GifMascot / null
- [ ] E2. Props：`renderer` / `face` / `visemeCode` / `size` / `idlePoseRotation` / `primaryColor` / `secondaryColor` / `customGifSrc`；GIF case 传入 `playing={face !== "idle"}`

---

## 阶段 F — mascotStore 改造（Step 5）

- [ ] F1. `mascotStore.ts` import 路径更新：`MascotFace` / `MascotRendererId` 从 `./core/types`
- [ ] F2. `MascotState` 新增 `renderer: MascotRendererId`（默认 `"rive"`）+ `customGifSrc: string | null`（默认 `null`）
- [ ] F3. 新增 `setRenderer` / `setCustomGifSrc` 方法
- [ ] F4. mascotStore 不持有渲染器内部状态

---

## 阶段 G — 消费端改造（Step 6）

- [ ] G1. `OrbBall.tsx`：`import { RiveMascot }` → `import { MascotRenderer }`；从 mascotStore 读取 `renderer`；`<RiveMascot ...>` → `<MascotRenderer renderer={renderer} ...>`
- [ ] G2. `MessageInput.tsx`：同上改造
- [ ] G3. 全局搜索所有 `from "../mascot/RiveMascot"` / `from "../../mascot/RiveMascot"` 引用，替换为 `MascotRenderer`

---

## 阶段 H — 旧文件清理（Step 7）

- [ ] H1. 删除 `src/mascot/faceStates.ts`（已被 `core/types.ts` 替代）
- [ ] H2. 删除 `src/mascot/riveMaps.ts`（已迁移至 `renderers/rive/riveMaps.ts`）
- [ ] H3. 删除 `src/mascot/RiveMascot.tsx`（已迁移至 `renderers/rive/RiveMascot.tsx`）
- [ ] H4. 全局搜索所有对旧文件的 import，更新路径

---

## 阶段 I — 公共 API + index.ts

- [ ] I1. 新建 `src/mascot/index.ts`：re-export `MascotRenderer` / `useMascotStore` / `useMascotChatSync` / `MascotFace` / `MascotRendererId` / `resolveVisemeCode` / `pickIdleFlourish`

---

## 阶段 J — 外观设置集成

- [ ] J1. 新建 `src/settings/MascotSection.tsx`：渲染方式 Select（Rive/SVG/GIF/禁用）+ 条件设置区（Rive: 主色调/辅色调/空闲动画；SVG: 空闲动画；GIF: 图片路径 + 局限性提示）
- [ ] J2. `AppearanceSettings.tsx` 新增"助手形象" SettingsSection，嵌入 MascotSection
- [ ] J3. Rust 后端：`assistant-config` 新增 `MascotConf` 结构体（renderer + primary_color + secondary_color + custom_gif_src）
- [ ] J4. Rust 后端：`mascot_conf.rs` load / save / get_conf_path（与 `orb_conf.rs` 同模式）
- [ ] J5. Rust 后端：`cmd.rs` 新增 `get_mascot_conf` / `save_mascot_conf` / `install_mascot_avatar` 命令（`install_mascot_avatar` 含大小限制 2MB + MIME 嗅探 + 扩展名校验，参考 deepseek-harness-orb `orb-avatar.ts`）
- [ ] J6. Rust 后端：`lib.rs` invoke_handler 注册新命令

---

## 阶段 K — 编译验证

- [ ] K1. `tsc --noEmit` 通过
- [ ] K2. `npm run lint` 0 errors
- [ ] K3. `cargo check` 通过

**依赖**：A1~A2, B1~B3, C1~C7, D1~D4, E1~E2, F1~F4, G1~G3, H1~H4, I1, J1~J6

---

## 阶段 L — 功能验证

- [ ] L1. `renderer === "rive"` 时行为与改造前完全一致（回归测试）
- [ ] L2. `renderer === "svg"` 时 GhostyMascot 正常显示，表情随 face 变化
- [ ] L3. `renderer === "gif"` 时 GifMascot 正常显示 GIF 动画；`face === "idle"` 时冻结到首帧，`face !== "idle"` 时播放
- [ ] L4. `renderer === "disabled"` 时无 Mascot 渲染
- [ ] L5. 外观设置"助手形象"区可切换渲染方式，立即生效
- [ ] L6. `useMascotChatSync` 表情联动正常（不受渲染器切换影响）

**依赖**：K1~K3

---

## 阶段 M — 验收标准

- [ ] M1. I5-extensible-renderer-spec.md §11 验收标准 1~18 项全部通过

**依赖**：L1~L6
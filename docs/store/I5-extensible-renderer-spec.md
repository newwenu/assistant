# I5-Ext — Mascot 可扩展渲染架构 规格说明

## 1. 交付目标

将 Mascot 从 Rive 单引擎硬耦合，重构为 **核心层 → 渲染器层 → 编排层** 三层分离架构，使外观设置可灵活切换渲染方式（Rive / SVG / GIF），且新增渲染引擎 **无需改动 Core 层和编排层代码**——渲染器通过注册表自注册，遵循开放-封闭原则。

> **前置依赖**：I5 ✅（MascotFace 10 种 + FACE_TO_POSE 映射 + RiveMascot + mascotStore + useMascotChatSync）
> **核心动机**：当前 `RiveMascot.tsx` + `riveMaps.ts` + `faceStates.ts` 三文件将类型、映射、渲染、状态混在一起，无法在不改代码的前提下切换渲染方式，也无法加载非内置 .riv 资产的表情映射。

### 当前架构问题

| 问题 | 位置 | 说明 |
|------|------|------|
| 类型与映射耦合 | `faceStates.ts` | `MascotFace` 与 `MascotRendererId` 混在同一文件 |
| 硬编码映射 | `riveMaps.ts` | `FACE_TO_POSE` 是代码常量，无法被自定义 .riv 资产覆盖 |
| 解析依赖 Rive 私有 | `riveMaps.ts` | `toRiveVisemeCode` 是 Rive 引擎的 viseme 别名映射，不应被引擎无关的解析逻辑依赖 |
| 渲染器与状态耦合 | `RiveMascot.tsx` | ambient 轮转逻辑 + pose 设置 + viseme 设置全在一个组件内 |
| 无渲染器调度 | `OrbBall.tsx` / `MessageInput.tsx` | 直接 import `RiveMascot`，切换渲染方式需改消费端代码 |
| 无外观设置入口 | `AppearanceSettings.tsx` | 无"助手形象"设置区，用户无法选择渲染方式 |

---

## 2. 三层架构

```
 ┌──────────────────────────────────────────────────┐
 │  Layer 2: Orchestration（编排层）                 │
 │  MascotRenderer.tsx  mascotStore.ts              │
 │  useMascotChatSync.ts                            │
 ├──────────────────────────────────────────────────┤
 │  Layer 1: Renderers（渲染器层）                   │
 │  renderers/rive/   renderers/svg/  renderers/gif/│
 ├──────────────────────────────────────────────────┤
 │  Layer 0: Core（核心层）                          │
 │  types.ts  resolve.ts  registry.ts               │
 │  跨引擎共享的类型 + 通用工具 + 渲染器注册表       │
 └──────────────────────────────────────────────────┘

  依赖方向：Layer N → Layer N-1（严格单向，禁止跨层依赖）
```

### 2.1 为什么三层而非五层

原方案将 Core 拆为 Protocol / Resolution / Built-in 三个子目录（五层），但：

- 前三个子层加起来约 116 行代码，却需要 9 个文件 + 3 个目录
- Protocol（类型）、Resolution（纯函数）、Built-in（纯数据）的关系是 **接口 → 算法 → 默认值**，在 TypeScript 项目中通常属于同一模块的三部分
- 依赖规则（"builtin 不依赖 resolution"）靠代码约定即可保证，不需要目录边界强制
- 合并为 `core/` 后，文件级依赖方向不变：`builtin.ts → types.ts`，`resolve.ts → types.ts`，两者互不依赖

如果未来 Core 层膨胀（远程 manifest 加载、第三方插件 SDK、多个内置配置），再按需拆分。

### 2.2 依赖规则

| 规则 | 说明 |
|------|------|
| 上层可依赖下层 | `renderers/rive/` 可 import `core/` |
| 下层不可依赖上层 | `core/` 不可 import `renderers/` 或 `MascotRenderer.tsx` |
| 同层不可交叉依赖 | `renderers/rive/` 不可 import `renderers/svg/`，反之亦然 |
| Core 不含渲染器特定概念 | `core/` 只放跨引擎共享的类型、通用工具和渲染器注册表 |
| mascotStore 不持有渲染器内部状态 | `stateEngine`、`FACE_PRESETS` 等是渲染器内部实现，不放在 store 中 |
| 渲染器 index.ts 可依赖 core/registry | 自注册是渲染器对 Core 的正常依赖（上层→下层），但渲染器组件本身不依赖 registry |

---

## 3. 目录结构

```
src/mascot/
  core/                              ← Layer 0：跨引擎共享的类型 + 通用工具 + 渲染器注册表
    types.ts                         MascotFace, MascotRendererId, RendererCapabilities, RendererMeta, MascotRendererProps
    resolve.ts                       resolveVisemeCode + pickIdleFlourish
    registry.ts                      registerRenderer + getRenderer + getAllRendererMetas + BUILTIN_RENDERERS
    index.ts                         re-export
  renderers/                          ← Layer 1：各引擎自治
    rive/
      RiveMascot.tsx                 Rive 渲染组件（从现有迁移）
      riveMaps.ts                    Rive 私有：pose 枚举、viseme 别名、状态机名
      riveStateEngine.ts             Rive 专有：RiveStateEngine 接口 + resolveFaceToPose + BUILTIN_RIVE_STATE_ENGINE
      index.ts
    svg/
      GhostyMascot.tsx               SVG 渲染组件（新增），内含 FACE_PRESETS / SVG_VISEME_CODES / SVG_IDLE_CYCLE
      index.ts
    gif/
      GifMascot.tsx                  GIF/图片渲染组件（新增）
      index.ts
  MascotRenderer.tsx                 ← Layer 2: 渲染器调度
  mascotStore.ts                     ← Layer 2: 状态管理
  useMascotChatSync.ts               ← Layer 2: chat→mascot 桥接
  index.ts                           公共 API
```

### 3.1 旧文件处置

| 旧文件 | 处置 | 原因 |
|--------|------|------|
| `faceStates.ts` | 删除 | `MascotFace` → `core/types.ts`，`MascotRendererId` → `core/types.ts` |
| `riveMaps.ts`（根目录） | 删除 | 迁移至 `renderers/rive/riveMaps.ts`，仅保留 Rive 私有内容 |
| `RiveMascot.tsx`（根目录） | 删除 | 迁移至 `renderers/rive/RiveMascot.tsx` |

---

## 4. 各层详细规格

### 4.1 Layer 0: Core — 跨引擎共享的类型 + 通用工具

#### `core/types.ts`

```typescript
export type MascotFace =
  | "idle" | "listening" | "thinking" | "speaking"
  | "happy" | "concerned" | "curious" | "working"
  | "waving" | "proud";

// ── 渲染器标识：开放品牌化字符串，遵循开放-封闭原则 ──
export type MascotRendererId = string & { __brand: "MascotRendererId" };

export function asRendererId(id: string): MascotRendererId {
  return id as MascotRendererId;
}

export const BUILTIN_RENDERER_IDS = {
  rive:     asRendererId("rive"),
  svg:      asRendererId("svg"),
  gif:      asRendererId("gif"),
  disabled: asRendererId("disabled"),
} as const;

// ── 渲染器能力声明 ──
export interface RendererCapabilities {
  /** 是否支持口型同步（viseme） */
  visemeSync: boolean;
  /** 是否支持按 MascotFace 切换不同表情 */
  faceTransition: boolean;
  /** 是否支持主/辅色调自定义 */
  colorCustomization: boolean;
  /** 是否支持空闲动画轮转 */
  idleAnimation: boolean;
}

// ── 渲染器元数据（注册时提供） ──
export interface RendererMeta {
  id: MascotRendererId;
  label: string;
  capabilities: RendererCapabilities;
  /** 局限性提示文本，设置 UI 中显示（如 GIF 的"不会响应对话状态变化"） */
  limitations?: string;
}

// ── 统一渲染器协议 Props ──
export interface MascotRendererProps {
  face: MascotFace;
  size?: number | string;
  visemeCode?: string;
  /** 渲染器自治的扩展配置，各渲染器内部自行提取所需字段 */
  rendererConfig?: Record<string, unknown>;
}
```

- `MascotFace` 是表情词汇表——所有渲染器都消费这个类型，但各自决定如何解释
- `MascotRendererId` 是品牌化字符串——内置渲染器通过 `BUILTIN_RENDERER_IDS` 引用，第三方渲染器通过 `asRendererId()` 创建，**无需修改 Core 即可扩展**
- `RendererCapabilities` 声明渲染器能力——编排层和设置 UI 据此自适应（如不支持 `visemeSync` 则不传 `visemeCode`，设置 UI 自动隐藏口型相关选项）
- `RendererMeta` 是注册时提供的元数据——包含 id、显示名、能力声明、局限性提示
- `MascotRendererProps` 是统一协议——各渲染器必须接受此接口，扩展配置通过 `rendererConfig` 传递，避免编排层感知渲染器专属 props

#### `core/resolve.ts`

两个通用工具函数，零副作用：

##### `resolveVisemeCode` — viseme 校验回退

```typescript
export function resolveVisemeCode(
  code: string,
  visemeCodes: string[],
): string {
  const lower = code.toLowerCase();
  const match = visemeCodes.find((c) => c.toLowerCase() === lower);
  if (match) return match;
  const sil = visemeCodes.find((c) => c.toLowerCase() === "sil");
  return sil ?? visemeCodes[0];
}
```

**设计要点**：
- 参数是 `visemeCodes: string[]`——调用方传入自己引擎的口型码列表
- Rive 传入 15 种标准口型，SVG 传入 3-5 种简化口型，GIF 不调用

##### `pickIdleFlourish` — 空闲轮转选取

```typescript
export function pickIdleFlourish(
  cycle: string[],
  exclude?: string,
  rng: () => number = Math.random,
): string {
  const pool = cycle.filter((p) => p !== exclude);
  if (pool.length === 0) return cycle[0] ?? "";
  const idx = Math.min(pool.length - 1, Math.floor(rng() * pool.length));
  return pool[idx];
}
```

**设计要点**：
- 参数是 `cycle: string[]`——调用方传入自己的轮转列表
- Rive 传入 `idlePoseCycle`，SVG 传入 `SVG_IDLE_CYCLE`

#### `core/registry.ts`

渲染器注册表——Core 层唯一可变状态，支持运行时注册/查询，替代编排层的 switch-case 编译时绑定：

```typescript
import type {
  MascotRendererId, MascotRendererProps, RendererMeta, RendererCapabilities,
} from "./types";
import type { FC } from "react";

interface RendererEntry {
  meta: RendererMeta;
  component: FC<MascotRendererProps>;
  /** 渲染器专属设置面板组件，设置 UI 按当前 renderer 动态加载 */
  SettingsPanel?: FC;
  /** 渲染器资源释放函数，切换渲染器时调用 */
  dispose?: () => void;
}

const registry = new Map<string, RendererEntry>();

export function registerRenderer(entry: RendererEntry): void {
  if (registry.has(entry.meta.id as string)) {
    console.warn(`[mascot] renderer "${entry.meta.id}" already registered, overwriting`);
  }
  registry.set(entry.meta.id as string, entry);
}

export function getRenderer(id: MascotRendererId): RendererEntry | undefined {
  return registry.get(id as string);
}

export function getAllRendererMetas(): RendererMeta[] {
  return Array.from(registry.values()).map((e) => e.meta);
}

export function unregisterRenderer(id: MascotRendererId): void {
  const entry = registry.get(id as string);
  entry?.dispose?.();
  registry.delete(id as string);
}
```

**设计要点**：
- `registerRenderer` 在各渲染器 `index.ts` 中调用——渲染器自注册，编排层零改动
- `SettingsPanel` 允许渲染器自带设置 UI——设置页面从注册表动态生成，无需手动枚举
- `dispose` 约定渲染器生命周期——切换渲染器时自动调用旧渲染器的资源释放（如 Rive 的 WebGL 上下文清理）
- 注册表是 Core 层唯一可变状态，但它是**启动时写入、运行时只读**的，不违反 Core 层"零副作用"原则（副作用仅在模块初始化阶段）

**内置渲染器注册**（在各 `renderers/*/index.ts` 中执行）：

```typescript
// renderers/rive/index.ts
registerRenderer({
  meta: {
    id: BUILTIN_RENDERER_IDS.rive,
    label: "Rive",
    capabilities: { visemeSync: true, faceTransition: true, colorCustomization: true, idleAnimation: true },
  },
  component: RiveMascot,
  SettingsPanel: RiveSettingsPanel,
  dispose: () => { /* 清理 Rive WebGL 资源 */ },
});

// renderers/svg/index.ts
registerRenderer({
  meta: {
    id: BUILTIN_RENDERER_IDS.svg,
    label: "SVG",
    capabilities: { visemeSync: true, faceTransition: true, colorCustomization: false, idleAnimation: true },
  },
  component: GhostyMascot,
  SettingsPanel: SvgSettingsPanel,
});

// renderers/gif/index.ts
registerRenderer({
  meta: {
    id: BUILTIN_RENDERER_IDS.gif,
    label: "GIF",
    capabilities: { visemeSync: false, faceTransition: false, colorCustomization: false, idleAnimation: false },
    limitations: "GIF 模式下助手不会响应对话状态变化",
  },
  component: GifMascot,
  SettingsPanel: GifSettingsPanel,
});
```

---

### 4.2 Layer 1: Renderers — 各引擎自治

#### `renderers/rive/riveStateEngine.ts`

Rive 专有的状态引擎定义，仅 Rive 渲染器内部使用：

```typescript
import type { MascotFace } from "../../core/types";

export interface RiveChannel {
  key: string;
  values: string[];
  default?: string;
  cycle?: {
    enabled: boolean;
    intervalMs?: number;
    order?: "random" | "sequential";
  };
}

export interface RiveStateEngine {
  /** 该 .riv 资产接受的嘴型码列表 */
  visemeCodes: string[];
  /** MascotFace → RivePose 名称映射（至少含 idle 和 thinking） */
  states: Record<MascotFace, string> & { idle: string; thinking: string };
  /** 空闲轮转 pose 列表 */
  idlePoseCycle: string[];
  /** Rive ViewModel 额外通道（如眼睛方向） */
  channels?: RiveChannel[];
}

/** 内置 mascot.riv 的能力描述 */
export const BUILTIN_RIVE_STATE_ENGINE: RiveStateEngine = {
  visemeCodes: [
    "sil", "PP", "FF", "TH", "DD", "kk",
    "CH", "SS", "nn", "RR", "aa", "E",
    "ih", "oh", "ou",
  ],
  states: {
    idle: "idle",
    listening: "idle",
    thinking: "thinking",
    speaking: "writing",
    happy: "celebration",
    concerned: "thinking",
    curious: "bookreading",
    working: "writing",
    waving: "hand_wave",
    proud: "celebration",
  },
  idlePoseCycle: [
    "thinking", "bookreading", "coffeedrink",
    "writing", "bobbateadrink", "dancing", "hand_wave", "celebration",
  ],
};

/** Rive 专有的 face→pose 两级回退 */
export function resolveFaceToPose(
  face: MascotFace,
  engine: RiveStateEngine,
): string {
  const mapped = engine.states[face];
  if (mapped) return mapped;
  return engine.states.idle;
}
```

**设计要点**：
- `RiveStateEngine` 描述一个 .riv 资产"能接受什么 pose/viseme"——是资产描述，不是用户偏好
- `BUILTIN_RIVE_STATE_ENGINE.states` 与原 `FACE_TO_POSE` 语义一致
- `resolveFaceToPose` 是 Rive 专有——SVG 用 `FACE_PRESETS[face]` 直接索引，GIF 不需要映射

#### `renderers/rive/riveMaps.ts`

Rive 私有模块。保留内容：

| 导出 | 说明 |
|------|------|
| `MASCOT_STATE_MACHINE` | 状态机名称 `"MascotSM"` |
| `RIVE_POSES` / `RivePose` | 9 种 pose 枚举 |
| `toRiveVisemeCode` | IPA → Rive enum 别名映射（Rive 私有） |
| `pickVisemeCode` | 文本字符 → viseme（供 chat streaming 使用） |

**移除内容**：

| 移除项 | 原因 |
|--------|------|
| `FACE_TO_POSE` | 被 `MascotStateEngine.states` 替代 |
| `faceToPose()` | 被 `resolveFaceToPose()` 替代 |
| `AMBIENT_POSES` / `pickAmbientPose` | 被 `pickIdleFlourish()` 替代 |

#### `renderers/rive/RiveMascot.tsx`

从现有 `src/mascot/RiveMascot.tsx` 迁移，改造要点：

```typescript
import { resolveVisemeCode, pickIdleFlourish } from "../../core";
import type { MascotRendererProps } from "../../core/types";
import { resolveFaceToPose, BUILTIN_RIVE_STATE_ENGINE } from "./riveStateEngine";
import { toRiveVisemeCode } from "./riveMaps";

interface RiveMascotProps extends MascotRendererProps {
  stateEngine?: RiveStateEngine;
}

// 从 rendererConfig 提取 Rive 专属配置
const {
  primaryColor,
  secondaryColor,
  idlePoseRotation,
} = (rendererConfig ?? {}) as {
  primaryColor?: number;
  secondaryColor?: number;
  idlePoseRotation?: boolean;
};

const engine = stateEngine ?? BUILTIN_RIVE_STATE_ENGINE;

// 之前：
const basePose = faceToPose(face);
// 之后：
const basePose = resolveFaceToPose(face, engine);

// 之前：
setMouthVisemeCode(toRiveVisemeCode(visemeCode));
// 之后：
const riveCode = toRiveVisemeCode(visemeCode);              // Rive 私有别名
const resolved = resolveVisemeCode(riveCode, engine.visemeCodes); // 通用校验
setMouthVisemeCode(resolved);

// 之前（ambient 轮转）：
current = pickAmbientPose(current === "idle" ? undefined : current);
// 之后：
current = pickIdleFlourish(engine.idlePoseCycle, current === "idle" ? undefined : current);
```

**stateEngine 的归属**：`stateEngine` 是渲染器内部实现细节。RiveMascot 默认使用 `BUILTIN_RIVE_STATE_ENGINE`，未来加载远程 .riv 时由渲染器内部切换 engine。

#### `renderers/svg/GhostyMascot.tsx`

新增 SVG 渲染组件，参考 OpenHuman `Ghosty.tsx`：

```typescript
import type { MascotRendererProps } from "../../core/types";

interface GhostyMascotProps extends MascotRendererProps {}

// 从 rendererConfig 提取 SVG 专属配置
const { animated = true } = (rendererConfig ?? {}) as { animated?: boolean };
```

**实现要点**：
- 纯 SVG + `useMascotClock` RAF 驱动（参考 OpenHuman Ghosty.tsx 的 `useMascotClock`）
- SVG path 变换实现 bob（上下浮动）/ blink（眨眼）/ wave（挥手）
- SVG mouth path 变换实现 viseme（open/closed/rounded 三种嘴型）
- 不依赖任何第三方动画库
- 使用 `resolveVisemeCode(code, SVG_VISEME_CODES)` 做 viseme 校验
- 不支持的 MascotFace 用 `FACE_PRESETS[face] ?? FACE_PRESETS.idle` 回退

**SVG 自治数据**（组件内部常量，不暴露到 Core）：

```typescript
const SVG_VISEME_CODES = ["sil", "open", "rounded", "closed"];
const SVG_IDLE_CYCLE = ["bob", "wave", "lookAround"];
const FACE_PRESETS: Record<MascotFace, FacePreset> = {
  idle:     { eyeScaleX: 1,   eyeScaleY: 1,   browTilt: 0,  ... },
  thinking: { eyeScaleX: 0.8, eyeScaleY: 0.6, browTilt: -5, ... },
  // ... 10 种表情各自对应一组 SVG 参数
};
```

> **为什么 SVG 不用 `resolveFaceToPose`**：SVG 没有 pose 名称概念，`MascotFace` 直接索引 `FACE_PRESETS`。

#### `renderers/gif/GifMascot.tsx`

新增 GIF/图片渲染组件，参考 OpenHuman `CustomGifMascot.tsx` + deepseek-harness-orb `floating.js`：

```typescript
import type { MascotRendererProps } from "../../core/types";

interface GifMascotProps extends MascotRendererProps {}

// 从 rendererConfig 提取 GIF 专属配置
const { src, playing } = (rendererConfig ?? {}) as {
  src?: string;
  playing?: boolean;
};
```

**实现要点**：
- 一个 `<img>` + `data-face` 属性，圆形裁剪（`border-radius: 50%` + `object-fit: cover`）
- **play/still 二态控制**（参考 deepseek-harness-orb `syncGif`）：
  - `playing=true`：`img.src = src`，GIF 正常播放
  - `playing=false`：`freezeGif()` 冻结到首帧，节省 CPU
- **冻结技术**：`canvas.drawImage(img, 0, 0) → canvas.toDataURL() → img.src = dataURL`，将 GIF 当前帧转为静态 PNG，零第三方依赖
- `src` 为空时显示占位图标
- MascotRenderer 传入 `playing` 的逻辑：`face !== "idle"` 时播放，`face === "idle"` 时冻结

> **⚠️ GIF 模式局限性**：GIF 只有"播放/冻结"两种状态，**无法按 face 切换不同表情**（用户说话时不会切换到 speaking 表情）且**无法做口型同步**。设置 UI 中应标注"GIF 模式下助手不会响应对话状态变化"，管理用户预期。

---

### 4.3 Layer 2: Orchestration — 编排

#### `MascotRenderer.tsx`

渲染器调度组件，从注册表查找渲染器并渲染，**零 switch-case**：

```typescript
import type { MascotRendererId } from "./core/types";
import { getRenderer } from "./core/registry";
import { BUILTIN_RENDERER_IDS } from "./core/types";

// 触发各渲染器自注册（副作用 import，仅执行一次）
import "./renderers/rive";
import "./renderers/svg";
import "./renderers/gif";

interface Props {
  renderer: MascotRendererId;
  face: MascotFace;
  visemeCode?: string;
  size?: number | string;
  rendererConfig?: Record<string, unknown>;
}

export const MascotRenderer: FC<Props> = (props) => {
  if (props.renderer === BUILTIN_RENDERER_IDS.disabled) return null;

  const entry = getRenderer(props.renderer);
  if (!entry) {
    console.warn(`[mascot] unknown renderer "${props.renderer}", falling back to rive`);
    const fallback = getRenderer(BUILTIN_RENDERER_IDS.rive);
    if (!fallback) return null;
    const Component = fallback.component;
    return <Component face={props.face} size={props.size}
      visemeCode={props.visemeCode} rendererConfig={props.rendererConfig} />;
  }

  const Component = entry.component;
  const { capabilities } = entry.meta;

  return <Component face={props.face} size={props.size}
    visemeCode={capabilities.visemeSync ? props.visemeCode : undefined}
    rendererConfig={props.rendererConfig} />;
};
```

**设计要点**：
- **注册表驱动**：`getRenderer()` 从注册表查找，新增渲染器无需改此文件
- **副作用 import**：`import "./renderers/rive"` 触发 `registerRenderer()`，确保渲染器在首次渲染前完成注册
- **能力自适应**：`capabilities.visemeSync` 为 false 时不传 `visemeCode`，避免渲染器收到无法处理的 prop
- **优雅降级**：未知 rendererId 自动回退到 Rive，不会白屏
- **生命周期**：`mascotStore.setRenderer()` 切换时，旧渲染器的 `dispose()` 由 `unregisterRenderer` 触发（仅在动态卸载场景）；正常切换时由 React 组件卸载触发 `useEffect` 清理

#### `mascotStore.ts` 改造

新增 2 个字段 + 2 个 setter：

```typescript
interface MascotState {
  // ... 现有字段 ...
  renderer: MascotRendererId;           // 新增，默认 "rive"
  customGifSrc: string | null;          // 新增，默认 null
  setRenderer: (id: MascotRendererId) => void;
  setCustomGifSrc: (src: string | null) => void;
}
```

**为什么 mascotStore 不持有渲染器内部状态**：

| 考量 | 说明 |
|------|------|
| 职责分离 | 资产描述（stateEngine / FACE_PRESETS）vs 用户偏好（renderer） |
| 持久化简化 | `MascotConf` 只存 `renderer` + 颜色 + `customGifSrc` |
| 渲染器自治 | 各渲染器自己知道用什么数据 |

#### `useMascotChatSync.ts`

**无需改造**。它只操作 `MascotFace`，与渲染器无关。

---

## 5. 外观设置集成

### 5.1 `settings/MascotSection.tsx` — 新增

设置 UI 从注册表动态生成，新增渲染器时**无需修改此文件**：

```typescript
import { getAllRendererMetas, getRenderer } from "../mascot/core/registry";

const metas = getAllRendererMetas();
const currentRenderer = useMascotStore((s) => s.renderer);
const entry = getRenderer(currentRenderer);
const SettingsPanel = entry?.SettingsPanel;

// 渲染方式下拉选项：从注册表动态生成
<Select value={currentRenderer} onChange={setRenderer}>
  {metas.map((m) => (
    <option key={m.id as string} value={m.id as string}>{m.label}</option>
  ))}
</Select>

// 当前渲染器的局限性提示（自动显示）
{entry?.meta.limitations && <Warning>{entry.meta.limitations}</Warning>}

// 当前渲染器的专属设置面板（动态加载）
{SettingsPanel && <SettingsPanel />}
```

**示意布局**（以 Rive 为当前选择）：

```
┌─────────────────────────────────────┐
│  助手形象                            │
│                                     │
│  渲染方式  [Rive ▼]                  │
│  选项来源：getAllRendererMetas()     │
│                                     │
│  ── Rive 设置 ── (SettingsPanel)    │
│  主色调    [■ #71c7df]              │
│  辅色调    [■ #e9f65c]              │
│  空闲动画  [✓] 轮转                  │
└─────────────────────────────────────┘
```

**切换到 GIF 时**（自动显示局限性提示）：

```
┌─────────────────────────────────────┐
│  助手形象                            │
│                                     │
│  渲染方式  [GIF ▼]                   │
│                                     │
│  ⚠ GIF 模式下助手不会响应对话状态变化 │
│                                     │
│  ── GIF 设置 ── (SettingsPanel)     │
│  图片路径  [选择文件...]             │
└─────────────────────────────────────┘
```

- 渲染方式选项从 `getAllRendererMetas()` 动态生成，新增渲染器自动出现
- 局限性提示从 `meta.limitations` 读取，无需硬编码
- 专属设置面板从 `entry.SettingsPanel` 动态加载，各渲染器自带
- 渲染方式切换时，`mascotStore.setRenderer()` 立即生效

### 5.2 `AppearanceSettings.tsx` 改造

在"悬浮球"和"悬浮窗"之间新增"助手形象"设置区：

```typescript
<SettingsSection
  title="助手形象"
  description="选择助手的显示方式和外观"
>
  <MascotSection />
</SettingsSection>
```

### 5.3 Rust 后端改造

`MascotConf` 只包含用户偏好，不包含 `stateEngine`：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MascotConf {
    pub renderer: String,               // "rive" | "svg" | "gif" | "disabled"
    pub primary_color: Option<u32>,     // ARGB
    pub secondary_color: Option<u32>,   // ARGB
    pub custom_gif_src: Option<String>, // 文件路径或 URL
}
```

后端改动范围：
- `assistant-config` crate 新增 `MascotConf` 结构体
- 独立 `mascot_conf.rs`：load / save / get_conf_path（与 `orb_conf.rs` 同模式）
- `cmd.rs` 新增 `get_mascot_conf` / `save_mascot_conf` 命令
- `lib.rs` invoke_handler 注册新命令

**自定义头像上传校验**（参考 deepseek-harness-orb `orb-avatar.ts`）：
- 文件大小限制（建议 2MB，与 deepseek-harness-orb 一致）
- MIME 类型嗅探 + 扩展名一致性校验（接受 GIF/PNG/WebP）
- 写入应用数据目录，`custom_gif_src` 存相对路径
- 缓存清除：URL 加 `?v=<mtime>` 参数

---

## 6. 消费端改造

### 6.1 `OrbBall.tsx`

```typescript
// 之前：
import { RiveMascot } from "../mascot/RiveMascot";
<RiveMascot face={mascotFace} size={ORB_HITBOX_SIZE} idlePoseRotation />

// 之后：
import { MascotRenderer } from "../mascot/MascotRenderer";
const renderer = useMascotStore((s) => s.renderer);
const primaryColor = useMascotStore((s) => s.primaryColor);
const secondaryColor = useMascotStore((s) => s.secondaryColor);
<MascotRenderer renderer={renderer} face={mascotFace}
  size={ORB_HITBOX_SIZE}
  rendererConfig={{ idlePoseRotation: true, primaryColor, secondaryColor }} />
```

### 6.2 `MessageInput.tsx`

```typescript
// 之前：
import { RiveMascot } from "../../mascot/RiveMascot";
<RiveMascot face={mascotFace} size={44} />

// 之后：
import { MascotRenderer } from "../../mascot/MascotRenderer";
const renderer = useMascotStore((s) => s.renderer);
<MascotRenderer renderer={renderer} face={mascotFace} size={44} />
```

---

## 7. 新增渲染器的标准流程

以新增 Lottie 为例：

| 步骤 | 操作 | 改动文件 |
|------|------|---------|
| 1 | 新建 `renderers/lottie/LottieMascot.tsx`，组件接受 `MascotRendererProps`，内部自行定义映射数据 | 1 文件 |
| 2 | 新建 `renderers/lottie/index.ts`，调用 `registerRenderer()` 自注册（含 meta + component + SettingsPanel + dispose） | 1 文件 |

**不需要改动的文件**：`core/types.ts`、`core/registry.ts`、`core/resolve.ts`、`MascotRenderer.tsx`、`MascotSection.tsx`、`mascotStore.ts`、`useMascotChatSync.ts`、其他渲染器。

> **渲染器自治 + 注册表驱动 = 开放-封闭原则**：新增渲染器只需在 `renderers/` 下新增 2 个文件并自注册，Core 层和编排层**零改动**。`MascotRenderer.tsx` 的副作用 import 会自动触发新渲染器的 `registerRenderer()`，`MascotSection.tsx` 的下拉选项从 `getAllRendererMetas()` 动态生成。

> **对比原方案**：原方案需 4 步 4 文件（改 types.ts 加 id + 新建组件 + 改 MascotRenderer switch + 改 MascotSection 选项），每次扩展都需改动 2 个稳定层文件。优化后扩展成本从 O(N) 降为 O(1)。

---

## 8. 依赖图（无环验证）

```
core/types ───────────────────────────────────────────────┐
core/resolve ── core/types ──────────────────────────────┤
core/registry ── core/types ─────────────────────────────┤
core/index ── core/types + core/resolve + core/registry ─┤
                                                           │
renderers/rive/riveStateEngine ── core/types ───────────┤
renderers/rive/riveMaps ── core/types ──────────────────┤
renderers/rive/RiveMascot ── core + ./riveStateEngine + ./riveMaps ─┤
renderers/rive/index ── core/registry + ./RiveMascot ───┤
renderers/svg/GhostyMascot ── core ─────────────────────┤
renderers/svg/index ── core/registry + ./GhostyMascot ──┤
renderers/gif/GifMascot ── core/types ──────────────────┤
renderers/gif/index ── core/registry + ./GifMascot ─────┤
                                                           │
MascotRenderer ── core/types + core/registry + renderers/* ─┤
mascotStore ── core/types ──────────────────────────────┤
useMascotChatSync ── mascotStore + core/types ──────────┘
```

所有箭头从上层指向下层，无环。`riveStateEngine` 不依赖 `riveMaps`（两者平行）。各渲染器 `index.ts` 依赖 `core/registry`（自注册），但渲染器组件本身不依赖 registry。

---

## 9. 不做的事（I5-Ext 范围外）

- ❌ 优先级调度器（idle/normal/force 三级）——当前无动画冲突场景，后续按需加入
- ❌ 动画事件总线（onPoseChange / onTransitionComplete）——当前无跨组件动画协调需求
- ❌ 自定义 .riv 文件上传 UI——RiveMascot 的 `stateEngine` prop 已预留接口，UI 在后续迭代
- ❌ 远程 .riv manifest 加载——RiveMascot 的 `stateEngine` prop 已预留，JSON manifest 解析在后续迭代
- ❌ Lottie / Spine / Live2D 渲染器——架构已支持扩展，具体引擎在后续按需实现
- ❌ CSS 动画 token 体系——与渲染器扩展正交，后续独立迭代
- ❌ GIF 多帧切换（按 face 切换不同 GIF）——增加复杂度，后续按需实现
- ❌ 渲染器懒加载（React.lazy + 代码分割）——注册表已预留 `component` 字段，可随时替换为 `React.lazy(() => import(...))`，当前三个渲染器体积小不需要

---

## 10. 参考项目

| 功能点 | 参考项目 | 参考文件 | 借鉴内容 |
|--------|---------|---------|---------|
| 多渲染器共存 | OpenHuman | `Mascot/index.ts` | RiveMascot / ManifestRiveMascot / CustomGifMascot / Ghosty 四种渲染器共存，共享 MascotFace 协议 |
| 数据驱动映射 | OpenHuman | `Mascot/manifest/stateEngine.ts` | `MascotStateEngine` 接口 + `resolveFaceToPose()` 三级回退（我们下沉为 Rive 专有，两级回退） |
| 远程 .riv manifest | OpenHuman | `Mascot/manifest/types.ts` | `MascotManifest` + `MascotStateEngine` + `MascotManifestChannel`，远程资产自带映射 |
| 纯 SVG 渲染器 | OpenHuman | `Mascot/Ghosty.tsx` | SVG + RAF 驱动，零依赖第三方动画库，bob/blink/wave/viseme 全部 SVG path 变换 |
| 最简 GIF 渲染器 | OpenHuman | `Mascot/CustomGifMascot.tsx` | `<img>` + `data-face`，GIF 自带动画 |
| React 组件即多态 | OpenHuman | `Mascot/index.ts` | 不用适配器模式，React 组件 + 共享 props 协议即渲染器抽象 |
| GIF play/still 控制 | deepseek-harness-orb | `renderer/floating.js` | `syncGif()` 二态控制：对话时播放，空闲时 `freezeGif()` 冻结到首帧省 CPU |
| GIF 冻结技术 | deepseek-harness-orb | `renderer/floating.js` | `canvas.drawImage → toDataURL` 将 GIF 当前帧转为静态 PNG，零第三方依赖 |
| 自定义头像上传 | deepseek-harness-orb | `src/orb-avatar.ts` | 大小限制 2MB + MIME 嗅探 + 扩展名一致性校验 + 写入 profile + 缓存清除 `?v=<mtime>` |
| 纯 GIF orb 生产实践 | deepseek-harness-orb | `renderer/floating.html` | 整个 orb 就是 `<img>` + 圆形裁剪，已在生产环境运行 |

---

## 11. 验收标准

1. `src/mascot/` 目录结构符合三层架构，无跨层依赖
2. `core/types.ts` 零逻辑、零副作用，仅导出 `MascotFace` + `MascotRendererId`（品牌化字符串）+ `RendererCapabilities` + `RendererMeta` + `MascotRendererProps`
3. `core/resolve.ts` 纯函数，参数为基本类型（`string[]`）
4. `core/registry.ts` 提供 `registerRenderer` / `getRenderer` / `getAllRendererMetas` / `unregisterRenderer`，注册表启动时写入、运行时只读
5. Core 不含渲染器特定概念（`RiveStateEngine` / `RiveChannel` / `BUILTIN_RIVE_STATE_ENGINE` 在 `renderers/rive/` 内）
6. `renderers/rive/riveStateEngine.ts` 持有 `RiveStateEngine` + `resolveFaceToPose` + `BUILTIN_RIVE_STATE_ENGINE`
7. `renderers/rive/` 不 import `renderers/svg/` 或 `renderers/gif/`，反之亦然
8. 各渲染器 `index.ts` 调用 `registerRenderer()` 自注册，含 `meta`（含 `capabilities`）+ `component` + `SettingsPanel`
9. `MascotRenderer.tsx` 从注册表查找渲染器，无 switch-case，未知 id 优雅降级到 Rive
10. `MascotRenderer.tsx` 根据 `capabilities.visemeSync` 决定是否传递 `visemeCode`
11. `renderer === "rive"` 时行为与改造前完全一致（回归测试）
12. `renderer === "svg"` 时 GhostyMascot 正常显示，表情随 face 变化
13. `renderer === "gif"` 时 GifMascot 正常显示 GIF 动画
14. `renderer === "gif"` 且 `face === "idle"` 时 GIF 冻结到首帧（省 CPU）
15. `renderer === "gif"` 且 `face !== "idle"` 时 GIF 播放
16. `renderer === "disabled"` 时无 Mascot 渲染
17. `MascotSection.tsx` 渲染方式选项从 `getAllRendererMetas()` 动态生成，局限性提示从 `meta.limitations` 读取，设置面板从 `entry.SettingsPanel` 动态加载
18. `mascotStore` 持有 `renderer` / `customGifSrc`，不持有渲染器内部状态
19. `useMascotChatSync` 无需改造，表情联动正常
20. 新增渲染器只需 2 步（新建组件 + 自注册 index.ts），Core 层和编排层零改动
21. 渲染器切换时旧渲染器的 `dispose()` 被调用（如有），资源正确释放
22. `cargo check` ✅ / `tsc --noEmit` ✅ / `lint 0 errors` ✅
# I5-Ext — Mascot 可扩展渲染架构 规格说明

## 1. 交付目标

将 Mascot 从 Rive 单引擎硬耦合，重构为 **协议层 → 解析层 → 内置配置层 → 渲染器层 → 编排层** 五层分离架构，使外观设置可灵活切换渲染方式（Rive / SVG / GIF），且新增渲染引擎无需改动编排逻辑。

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

## 2. 分层架构

```
 ┌──────────────────────────────────────────────────┐
 │  Layer 4: Orchestration（编排层）                 │
 │  MascotRenderer.tsx  mascotStore.ts              │
 │  useMascotChatSync.ts                            │
 ├──────────────────────────────────────────────────┤
 │  Layer 3: Renderers（渲染器层）                   │
 │  renderers/rive/   renderers/svg/  renderers/gif/│
 ├──────────────────────────────────────────────────┤
 │  Layer 2: Built-in Config（内置配置层）           │
 │  builtin/riveStateEngine.ts                      │
 ├──────────────────────────────────────────────────┤
 │  Layer 1: Resolution（解析层）                    │
 │  resolveFaceToPose  resolveVisemeCode            │
 │  pickIdleFlourish                                 │
 ├──────────────────────────────────────────────────┤
 │  Layer 0: Protocol（协议层）                      │
 │  MascotFace  MascotRendererId  MascotStateEngine │
 └──────────────────────────────────────────────────┘

  依赖方向：Layer N → Layer N-1（严格单向，禁止跨层依赖）
```

### 2.1 依赖规则

| 规则 | 说明 |
|------|------|
| 上层可依赖下层 | `renderers/rive/` 可 import `protocol/` + `resolution/` + `builtin/` |
| 下层不可依赖上层 | `protocol/` 不可 import `renderers/` |
| 同层不可交叉依赖 | `renderers/rive/` 不可 import `renderers/svg/`，反之亦然 |
| resolution 不持有数据 | 纯函数，所有数据通过参数注入 |
| builtin 不依赖 resolution | 纯数据，不调用解析函数 |

---

## 3. 目录结构

```
src/mascot/
  protocol/                          ← Layer 0
    types.ts                         MascotFace, MascotRendererId
    stateEngine.ts                   MascotStateEngine, MascotChannel 接口
    index.ts                         re-export
  resolution/                        ← Layer 1
    resolveFaceToPose.ts             face → pose 两级回退
    resolveVisemeCode.ts             viseme 码校验与回退
    pickIdleFlourish.ts              空闲轮转 pose 选取
    index.ts                         re-export
  builtin/                           ← Layer 2
    riveStateEngine.ts               BUILTIN_STATE_ENGINE 常量数据
    index.ts                         re-export
  renderers/                          ← Layer 3
    rive/
      RiveMascot.tsx                 Rive 渲染组件（从现有迁移）
      riveMaps.ts                    Rive 私有：pose 枚举、viseme 别名、状态机名
      index.ts
    svg/
      GhostyMascot.tsx               SVG 渲染组件（新增）
      index.ts
    gif/
      GifMascot.tsx                  GIF/图片渲染组件（新增）
      index.ts
  MascotRenderer.tsx                 ← Layer 4: 渲染器调度
  mascotStore.ts                     ← Layer 4: 状态管理
  useMascotChatSync.ts               ← Layer 4: chat→mascot 桥接
  index.ts                           公共 API
```

### 3.1 旧文件处置

| 旧文件 | 处置 | 原因 |
|--------|------|------|
| `faceStates.ts` | 删除 | `MascotFace` → `protocol/types.ts`，`MascotRendererId` → `protocol/types.ts` |
| `riveMaps.ts`（根目录） | 删除 | 迁移至 `renderers/rive/riveMaps.ts`，仅保留 Rive 私有内容 |
| `RiveMascot.tsx`（根目录） | 删除 | 迁移至 `renderers/rive/RiveMascot.tsx` |
| `stateEngine.ts`（根目录） | 删除 | 已删除，拆分为 `protocol/stateEngine.ts` + `resolution/*` + `builtin/riveStateEngine.ts` |

---

## 4. 各层详细规格

### 4.1 Layer 0: Protocol — 纯类型，零逻辑

#### `protocol/types.ts`

```typescript
export type MascotFace =
  | "idle" | "listening" | "thinking" | "speaking"
  | "happy" | "concerned" | "curious" | "working"
  | "waving" | "proud";

export type MascotRendererId = "rive" | "svg" | "gif" | "disabled";
```

#### `protocol/stateEngine.ts`

```typescript
import type { MascotFace } from "./types";

export interface MascotChannel {
  key: string;
  values: string[];
  default?: string;
  cycle?: {
    enabled: boolean;
    intervalMs?: number;
    order?: "random" | "sequential";
  };
}

export interface MascotStateEngine {
  /** 该引擎接受的嘴型码列表 */
  visemeCodes: string[];
  /** 逻辑表情 → pose 名称映射（至少含 idle 和 thinking） */
  states: Record<MascotFace, string> & { idle: string; thinking: string };
  /** 空闲轮转 pose 列表 */
  idlePoseCycle: string[];
  /** 额外动画通道（如眼睛方向） */
  channels?: MascotChannel[];
}
```

**设计要点**：
- `MascotStateEngine` 是数据驱动的核心——描述一个渲染引擎"能做什么"
- 不同 .riv 资产可以有不同的 `MascotStateEngine`（参考 OpenHuman ManifestRiveMascot）
- `states` 要求至少含 `idle` 和 `thinking`，这是所有引擎的最低保证

---

### 4.2 Layer 1: Resolution — 纯函数，零副作用

#### `resolution/resolveFaceToPose.ts`

**两级回退策略**：

| 级别 | 含义 | 场景 |
|------|------|------|
| 第 1 级 | 该引擎的显式映射 | `engine.states[face]` 有值 |
| 第 2 级 | 保底回退 | 返回 `engine.states.idle`（所有引擎必须定义） |

```typescript
import type { MascotFace } from "../protocol/types";
import type { MascotStateEngine } from "../protocol/stateEngine";

export function resolveFaceToPose(
  face: MascotFace,
  engine: MascotStateEngine,
): string {
  const mapped = engine.states[face];
  if (mapped) return mapped;
  return engine.states.idle;
}
```

**不采用三级回退的理由**：
- 当前项目只有一个内置 mascot.riv，第 1 级永远命中
- 即使未来支持自定义 .riv，如果资产作者没映射某个 face，他的 .riv 大概率也没有对应 pose
- 两级回退足够保证不崩溃，且解析层零数据依赖（不需要 `builtinFallback` 参数）

#### `resolution/resolveVisemeCode.ts`

```typescript
import type { MascotStateEngine } from "../protocol/stateEngine";

export function resolveVisemeCode(
  code: string,
  engine: MascotStateEngine,
): string {
  const lower = code.toLowerCase();
  const match = engine.visemeCodes.find((c) => c.toLowerCase() === lower);
  if (match) return match;

  const sil = engine.visemeCodes.find((c) => c.toLowerCase() === "sil");
  return sil ?? engine.visemeCodes[0];
}
```

**设计要点**：
- **不依赖 `toRiveVisemeCode`**——那是 Rive 私有的别名映射，不属于解析层
- Rive 渲染器调用链：`toRiveVisemeCode(visemeCode)` → `resolveVisemeCode(result, engine)`
- SVG 渲染器调用链：`resolveVisemeCode(visemeCode, engine)`（SVG 可能只支持 sil/non-sil 二值）

#### `resolution/pickIdleFlourish.ts`

```typescript
import type { MascotStateEngine } from "../protocol/stateEngine";

export function pickIdleFlourish(
  engine: MascotStateEngine,
  exclude?: string,
  rng: () => number = Math.random,
): string {
  const rest = engine.states.idle;
  const pool = engine.idlePoseCycle.filter((p) => p !== rest && p !== exclude);
  const choices = pool.length > 0 ? pool : engine.idlePoseCycle.filter((p) => p !== rest);
  if (choices.length === 0) return rest;
  const idx = Math.min(choices.length - 1, Math.floor(rng() * choices.length));
  return choices[idx];
}
```

---

### 4.3 Layer 2: Built-in Config — 纯数据

#### `builtin/riveStateEngine.ts`

```typescript
import type { MascotStateEngine } from "../protocol/stateEngine";

export const BUILTIN_STATE_ENGINE: MascotStateEngine = {
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
    "idle", "thinking", "bookreading", "coffeedrink",
    "writing", "bobbateadrink", "dancing", "hand_wave", "celebration",
  ],
};
```

**设计要点**：
- `BUILTIN_STATE_ENGINE` 是内置 `mascot.riv` 的能力描述——**数据即文档**
- `states` 字段与原 `FACE_TO_POSE` 语义完全一致，只是从函数映射变为数据映射
- 未来远程 .riv 资产自带 JSON manifest，替换此内置配置

---

### 4.4 Layer 3: Renderers — 各引擎自治

#### `renderers/rive/riveMaps.ts`

Rive 私有模块，不对外暴露。保留内容：

| 导出 | 说明 |
|------|------|
| `MASCOT_STATE_MACHINE` | 状态机名称 `"MascotSM"` |
| `RIVE_POSES` / `RivePose` | 9 种 pose 枚举 |
| `toRiveVisemeCode` | IPA → Rive enum 别名映射（Rive 私有） |
| `pickVisemeCode` | 文本字符 → viseme（供 chat streaming 使用） |
| `AMBIENT_POSES` / `pickAmbientPose` | ambient 轮转列表（Rive 私有，被 `pickIdleFlourish` 替代，保留供兼容过渡） |

**移除内容**：

| 移除项 | 原因 |
|--------|------|
| `FACE_TO_POSE` | 被 `MascotStateEngine.states` 替代 |
| `faceToPose()` | 被 `resolveFaceToPose()` 替代 |

#### `renderers/rive/RiveMascot.tsx`

从现有 `src/mascot/RiveMascot.tsx` 迁移，改造要点：

```typescript
// 新增 import
import { resolveFaceToPose, resolveVisemeCode, pickIdleFlourish } from "../../resolution";
import { BUILTIN_STATE_ENGINE } from "../../builtin";
import { toRiveVisemeCode } from "./riveMaps";

// 新增 prop
interface RiveMascotProps {
  // ... 现有 props ...
  stateEngine?: MascotStateEngine;    // 新增：允许传入自定义引擎配置
}

// 组件内改造
const engine = stateEngine ?? BUILTIN_STATE_ENGINE;

// 之前：
const basePose = faceToPose(face);
// 之后：
const basePose = resolveFaceToPose(face, engine);

// 之前：
setMouthVisemeCode(toRiveVisemeCode(visemeCode));
// 之后：
const riveCode = toRiveVisemeCode(visemeCode);       // Rive 私有别名
const resolved = resolveVisemeCode(riveCode, engine); // 引擎无关校验
setMouthVisemeCode(resolved);

// 之前（ambient 轮转）：
current = pickAmbientPose(current === "idle" ? undefined : current);
// 之后：
current = pickIdleFlourish(engine, current === "idle" ? undefined : current);
```

#### `renderers/svg/GhostyMascot.tsx`

新增 SVG 渲染组件，参考 OpenHuman `Ghosty.tsx`：

```typescript
interface GhostyMascotProps {
  face?: MascotFace;
  size?: number | string;
  visemeCode?: string;
  animated?: boolean;           // 是否启用 bob/blink 动画
  stateEngine?: MascotStateEngine;
}
```

**实现要点**：
- 纯 SVG + `useMascotClock` RAF 驱动（参考 OpenHuman Ghosty.tsx 的 `useMascotClock`）
- SVG path 变换实现 bob（上下浮动）/ blink（眨眼）/ wave（挥手）
- SVG mouth path 变换实现 viseme（open/closed/rounded 三种嘴型）
- 不依赖任何第三方动画库
- 使用 `resolveVisemeCode` 做 viseme 校验

#### `renderers/gif/GifMascot.tsx`

新增 GIF/图片渲染组件，参考 OpenHuman `CustomGifMascot.tsx`：

```typescript
interface GifMascotProps {
  src: string;
  face?: MascotFace;
  size?: number | string;
}
```

**实现要点**：
- 最简渲染器：一个 `<img>` + `data-face` 属性
- GIF 自带动画，不需要 JS 驱动
- `src` 为空时显示占位图标

---

### 4.5 Layer 4: Orchestration — 编排

#### `MascotRenderer.tsx`

渲染器调度组件，根据 `renderer` 切换渲染器：

```typescript
import type { MascotFace, MascotRendererId } from "./protocol/types";
import type { MascotStateEngine } from "./protocol/stateEngine";
import { RiveMascot } from "./renderers/rive";
import { GhostyMascot } from "./renderers/svg";
import { GifMascot } from "./renderers/gif";

interface Props {
  renderer: MascotRendererId;
  face: MascotFace;
  visemeCode?: string;
  size?: number | string;
  idlePoseRotation?: boolean;
  stateEngine?: MascotStateEngine;
  primaryColor?: number;
  secondaryColor?: number;
  customGifSrc?: string;
}

export const MascotRenderer: FC<Props> = (props) => {
  switch (props.renderer) {
    case "rive":
      return <RiveMascot face={props.face} size={props.size}
        visemeCode={props.visemeCode} idlePoseRotation={props.idlePoseRotation}
        stateEngine={props.stateEngine}
        primaryColor={props.primaryColor} secondaryColor={props.secondaryColor} />;
    case "svg":
      return <GhostyMascot face={props.face} size={props.size}
        visemeCode={props.visemeCode} animated={props.idlePoseRotation}
        stateEngine={props.stateEngine} />;
    case "gif":
      return <GifMascot src={props.customGifSrc ?? ""} face={props.face} size={props.size} />;
    case "disabled":
      return null;
  }
};
```

#### `mascotStore.ts` 改造

新增 3 个字段 + 3 个 setter：

```typescript
interface MascotState {
  // ... 现有字段 ...
  renderer: MascotRendererId;           // 新增，默认 "rive"
  stateEngine: MascotStateEngine;       // 新增，默认 BUILTIN_STATE_ENGINE
  customGifSrc: string | null;          // 新增，默认 null
  setRenderer: (id: MascotRendererId) => void;
  setStateEngine: (engine: MascotStateEngine) => void;
  setCustomGifSrc: (src: string | null) => void;
}
```

#### `useMascotChatSync.ts`

**无需改造**。它只操作 `MascotFace`，与渲染器无关。

---

## 5. 外观设置集成

### 5.1 `settings/MascotSection.tsx` — 新增

```
┌─────────────────────────────────────┐
│  助手形象                            │
│                                     │
│  渲染方式  [Rive ▼]                  │
│                                     │
│  ── Rive 设置 ──                    │
│  主色调    [■ #6366F1]              │
│  辅色调    [■ #8B5CF6]              │
│  空闲动画  [✓] 轮转                  │
│                                     │
│  ── GIF 设置 ── (渲染方式=GIF时显示) │
│  图片路径  [选择文件...]             │
└─────────────────────────────────────┘
```

- 渲染方式切换时，`mascotStore.setRenderer()` 立即生效
- Rive 设置区仅 `renderer === "rive"` 时显示
- GIF 设置区仅 `renderer === "gif"` 时显示
- SVG 无额外设置（纯内置动画）

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

### 5.3 `settingsStore.ts` 改造

新增 `mascotConf`（与 `orbConf` 同级）：

```typescript
export interface MascotConf {
  renderer: MascotRendererId;
  primary_color: number | null;
  secondary_color: number | null;
  custom_gif_src: string | null;
}
```

### 5.4 Rust 后端改造

- `assistant-config` crate 新增 `MascotConf` 结构体
- `app_conf.rs` 或独立 `mascot_conf.rs`：load / save / get_conf_path
- `cmd.rs` 新增 `get_mascot_conf` / `save_mascot_conf` 命令
- `lib.rs` invoke_handler 注册新命令

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
const stateEngine = useMascotStore((s) => s.stateEngine);
<MascotRenderer renderer={renderer} face={mascotFace}
  size={ORB_HITBOX_SIZE} idlePoseRotation stateEngine={stateEngine} />
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
| 1 | `protocol/types.ts` 的 `MascotRendererId` 加 `"lottie"` | 1 文件 |
| 2 | 新建 `renderers/lottie/LottieMascot.tsx` + `index.ts` | 2 文件 |
| 3 | `MascotRenderer.tsx` switch 加 `"lottie"` case | 1 文件 |
| 4 | `MascotSection.tsx` 渲染方式选项加 "Lottie" | 1 文件 |

**不需要改动的文件**：`mascotStore.ts`、`useMascotChatSync.ts`、`resolution/*`、`builtin/*`、其他渲染器。

---

## 8. 依赖图（无环验证）

```
protocol/types ──────────────────────────────────────┐
protocol/stateEngine ── protocol/types ──────────────┤
                                                      │
resolution/resolveFaceToPose ── protocol ────────────┤
resolution/resolveVisemeCode ── protocol ────────────┤
resolution/pickIdleFlourish ── protocol ─────────────┤
                                                      │
builtin/riveStateEngine ── protocol ─────────────────┤
                                                      │
renderers/rive/riveMaps ── protocol/types ───────────┤
renderers/rive/RiveMascot ── protocol + resolution   │
                         ── builtin + ./riveMaps ────┤
renderers/svg/GhostyMascot ── protocol + resolution ─┤
renderers/gif/GifMascot ── protocol/types ───────────┤
                                                      │
MascotRenderer ── protocol + renderers/* ────────────┤
mascotStore ── protocol + builtin ───────────────────┤
useMascotChatSync ── mascotStore + protocol ─────────┘
```

所有箭头从上层指向下层，无环。

---

## 9. 不做的事（I5-Ext 范围外）

- ❌ 优先级调度器（idle/normal/force 三级）——当前无动画冲突场景，后续按需加入
- ❌ 动画事件总线（onPoseChange / onTransitionComplete）——当前无跨组件动画协调需求
- ❌ 自定义 .riv 文件上传 UI——`buffer` 属性已预留接口，UI 在后续迭代
- ❌ 远程 .riv manifest 加载——`stateEngine` 参数已预留，JSON manifest 解析在后续迭代
- ❌ Lottie / Spine / Live2D 渲染器——架构已支持扩展，具体引擎在后续按需实现
- ❌ CSS 动画 token 体系——与渲染器扩展正交，后续独立迭代

---

## 10. 参考项目

| 功能点 | 参考项目 | 参考文件 | 借鉴内容 |
|--------|---------|---------|---------|
| 多渲染器共存 | OpenHuman | `Mascot/index.ts` | RiveMascot / ManifestRiveMascot / CustomGifMascot / Ghosty 四种渲染器共存，共享 MascotFace 协议 |
| 数据驱动映射 | OpenHuman | `Mascot/manifest/stateEngine.ts` | `MascotStateEngine` 接口 + `resolveFaceToPose()` 三级回退（我们简化为两级） |
| 远程 .riv manifest | OpenHuman | `Mascot/manifest/types.ts` | `MascotManifest` + `MascotStateEngine` + `MascotManifestChannel`，远程资产自带映射 |
| 纯 SVG 渲染器 | OpenHuman | `Mascot/Ghosty.tsx` | SVG + RAF 驱动，零依赖第三方动画库，bob/blink/wave/viseme 全部 SVG path 变换 |
| 最简 GIF 渲染器 | OpenHuman | `Mascot/CustomGifMascot.tsx` | `<img>` + `data-face`，GIF 自带动画 |
| React 组件即多态 | OpenHuman | `Mascot/index.ts` | 不用适配器模式，React 组件 + 共享 props 协议即渲染器抽象 |

---

## 11. 验收标准

1. `src/mascot/` 目录结构符合五层架构，无跨层依赖
2. `protocol/` 零逻辑、零副作用，仅导出类型
3. `resolution/` 纯函数，不持有数据，不依赖 Rive 私有模块
4. `builtin/` 纯数据，不调用解析函数
5. `renderers/rive/` 不 import `renderers/svg/` 或 `renderers/gif/`，反之亦然
6. `MascotRenderer.tsx` 根据 `renderer` 正确切换渲染器
7. `renderer === "rive"` 时行为与改造前完全一致（回归测试）
8. `renderer === "svg"` 时 GhostyMascot 正常显示，表情随 face 变化
9. `renderer === "gif"` 时 GifMascot 正常显示 GIF 动画
10. `renderer === "disabled"` 时无 Mascot 渲染
11. 外观设置"助手形象"区可切换渲染方式，立即生效
12. `mascotStore.renderer` / `stateEngine` / `customGifSrc` 字段可读写
13. `useMascotChatSync` 无需改造，表情联动正常
14. 新增渲染器只需 4 步（types + 组件 + switch case + 设置选项），无需改 store / resolution / builtin
15. `cargo check` ✅ / `tsc --noEmit` ✅ / `lint 0 errors` ✅
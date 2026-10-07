# I5 — Mascot 形象 + 表情联动 规格说明

## 1. 交付目标

Rive 动画 Mascot 与对话状态完整联动：悬浮球内 Mascot 表情随对话实时变化，发送按钮 Mascot 联动口型，空闲时自动轮转 ambient pose 保持活力感。

> **前置依赖**：I2 ✅（chatStore 流式输出 + ChatError）+ I4 ✅（OrbConf 外观配置）
> **当前状态**：核心 Mascot 模块已提前实现（RiveMascot / faceStates / riveMaps / mascotStore / useMascotChatSync），I5 聚焦于补齐与参考项目对齐的缺口 + 增强交互细节。

### 已实现清单（I1~I4 期间提前完成）

| 功能 | 文件 | 状态 |
|------|------|------|
| Rive 运行时集成 | `@rive-app/react-webgl2` ^4.36.0 | ✅ |
| .riv 动画资源 | `public/mascot.riv`（145KB） | ✅ |
| 9 种 MascotFace 枚举 | `faceStates.ts` | ✅ |
| 9 种 RivePose + FACE_TO_POSE 映射 | `riveMaps.ts` | ✅ |
| 15 种 Oculus viseme + alias 归一化 | `riveMaps.ts` | ✅ |
| ambient pose 轮转（6~12s 间隔） | `riveMaps.ts` + `RiveMascot.tsx` | ✅ |
| pickVisemeCode 文字→口型映射 | `riveMaps.ts` | ✅ |
| mascotStore（face/visemeCode/holdThenIdle） | `mascotStore.ts` | ✅ |
| useMascotChatSync 对话→表情联动 | `useMascotChatSync.ts` | ✅ |
| 悬浮球内 Mascot 显示 | `OrbBall.tsx` L195 | ✅ |
| 发送按钮 Mascot 显示 | `MessageInput.tsx` L83 | ✅ |
| Mascot CSS 样式 | `global.css` .chat-mascot-btn 系列 | ✅ |

### 缺口清单（I5 需补齐）

| 缺口 | 说明 | 参考 |
|------|------|------|
| RiveMascot 颜色换肤 | 缺 `primaryColor` / `secondaryColor` 属性 + `useViewModelInstanceColor` | OpenHuman RiveMascot.tsx L48-49 |
| RiveMascot buffer 加载 | 缺 `buffer?: ArrayBuffer` 属性，无法从内存加载自定义 .riv | OpenHuman RiveMascot.tsx L56 |
| MascotFace 扩展至 18 种 | 缺 sleep / confused / proud / cautious / celebrating / writing / reading / dancing / drinking_coffee / drinking_boba / normal 共 9 种 | OpenHuman Ghosty.tsx L20-38 |
| FACE_TO_POSE 完整映射 | 新增 9 种 face → pose 映射 | OpenHuman riveMaps.ts L48-67 |
| useMascotChatSync listening 状态 | 用户输入时未设 `listening` 表情 | ARCHITECTURE.md §3.4 |
| useMascotChatSync working 状态 | Agent 工具调用时未设 `working` 表情 | OpenBitFun agentCompanionMood.ts |
| mascotStore deriveMascotFaceFromChat 扩展 | 当前仅 3 种分支（error/streaming/reasoning），需扩展 listening/working | — |

## 2. 功能规格

### 2.1 RiveMascot 组件增强

参考 OpenHuman `RiveMascot.tsx`，补齐颜色换肤和内存加载：

```typescript
interface RiveMascotProps {
  face?: MascotFace;
  size?: number | string;
  primaryColor?: number;       // 新增：Rive ViewModel primaryColor
  secondaryColor?: number;     // 新增：Rive ViewModel secondaryColor
  visemeCode?: string;
  idlePoseRotation?: boolean;
  src?: string;
  buffer?: ArrayBuffer;        // 新增：内存加载自定义 .riv，优先于 src
  stateMachine?: string;
}
```

**实现要点**：
- `buffer` 优先于 `src`：`const riveSource = buffer ? { buffer } : { src }`，传给 `useRive` 避免双加载竞争
- `useViewModelInstanceColor('primaryColor', vmInstance)` + `useViewModelInstanceColor('secondaryColor', vmInstance)` 驱动 Rive 颜色属性
- `primaryColor` / `secondaryColor` 变更时 `useEffect` 调用 `setPrimaryColor` / `setSecondaryColor`

### 2.2 MascotFace 扩展

从 9 种扩展至 18 种，与 OpenHuman `Ghosty.tsx` 对齐：

```typescript
export type MascotFace =
  | 'sleep'
  | 'idle'
  | 'listening'
  | 'thinking'
  | 'confused'
  | 'speaking'
  | 'happy'
  | 'concerned'
  | 'curious'
  | 'proud'
  | 'cautious'
  | 'celebrating'
  | 'writing'
  | 'reading'
  | 'waving'
  | 'dancing'
  | 'drinking_coffee'
  | 'drinking_boba'
  | 'normal';   // legacy alias for idle
```

**新增 10 种表情语义**：

| MascotFace | 语义 | 触发场景 |
|------------|------|---------|
| `sleep` | 休眠 | 长时间无交互（>5min） |
| `confused` | 困惑 | Agent 迭代调用工具 / 多轮推理 |
| `proud` | 自豪 | 复杂任务完成（含工具调用） |
| `cautious` | 谨慎 | 轻微警告，不如 concerned 严重 |
| `celebrating` | 庆祝 | 重要任务成功完成 |
| `writing` | 写作 | Agent 编辑/创建文件 |
| `reading` | 阅读 | Agent 浏览/读取内容 |
| `dancing` | 跳舞 | 庆祝/ playful 动画 |
| `drinking_coffee` | 喝咖啡 | 长时间处理中 |
| `drinking_boba` | 喝奶茶 | 处理中的轻松变体 |
| `normal` | 旧别名 | 向后兼容，映射到 idle |

### 2.3 FACE_TO_POSE 完整映射

```typescript
export const FACE_TO_POSE: Record<MascotFace, RivePose> = {
  idle:            'idle',
  normal:          'idle',          // legacy alias
  sleep:           'idle',
  listening:       'idle',
  thinking:        'thinking',
  confused:        'thinking',
  speaking:        'idle',
  happy:           'idle',
  concerned:       'thinking',
  curious:         'bookreading',
  proud:           'celebration',
  cautious:        'thinking',
  celebrating:     'celebration',
  writing:         'writing',
  reading:         'bookreading',
  waving:          'hand_wave',
  dancing:         'dancing',
  drinking_coffee: 'coffeedrink',
  drinking_boba:   'bobbateadrink',
};
```

> 18 种 face 映射到 9 种 RivePose，多对一映射——Rive 资产仅 9 种动画，但 face 语义更丰富供后续扩展。

### 2.4 表情状态机增强

#### 2.4.1 useMascotChatSync 扩展

当前仅处理 streaming / reasoning / error 三种状态，需扩展：

| 对话事件 | 当前行为 | 增强行为 |
|---------|---------|---------|
| 用户开始输入 | 无 | `setFace('listening')` |
| 用户停止输入（2s 无输入） | 无 | 回到 `idle` |
| 流式输出开始（reasoning） | `setFace('thinking')` | 不变 |
| 流式输出开始（非 reasoning） | `setFace('speaking')` | 不变 |
| reasoning 结束 → 继续流式 | `setFace('speaking')` | 不变 |
| 流式输出结束 | `holdThenIdle('happy')` | 根据 ack 文本判断：error→concerned / 完成→happy / 工具调用→proud |
| 错误出现 | `holdThenIdle('concerned')` | 不变 |

**listening 状态实现**：

- MessageInput 的 `onChange` 事件 → `useMascotStore.getState().setFace('listening')`
- 2s 防抖：停止输入后自动回到 `idle`（仅在非 streaming 时）
- 发送消息时立即切换到 `thinking`（等待首 token），覆盖 listening

#### 2.4.2 deriveMascotFaceFromChat 扩展

```typescript
export function deriveMascotFaceFromChat(
  isStreaming: boolean,
  isReasoningStreaming: boolean,
  hasError: boolean,
  isListening?: boolean,    // 新增
): MascotFace {
  if (hasError) return 'concerned';
  if (isStreaming) return 'speaking';
  if (isReasoningStreaming) return 'thinking';
  if (isListening) return 'listening';
  return 'idle';
}
```

#### 2.4.3 pickAckFace 增强

当前仅检测 error / happy 两种 ack 表情，增加 proud 判断：

```typescript
function pickAckFace(fullText: string, hadToolCall?: boolean): MascotFace | null {
  if (!fullText.trim()) return null;
  if (CONCERNED_RE.test(fullText)) return 'concerned';
  if (hadToolCall && HAPPY_RE.test(fullText)) return 'proud';    // 含工具调用+成功 → proud
  if (HAPPY_RE.test(fullText)) return 'happy';
  if (hadToolCall) return 'happy';                                // 含工具调用但无明确成功词 → 仍 happy
  return null;
}
```

### 2.5 mascotStore 扩展

新增 `listening` 防抖支持：

```typescript
interface MascotState {
  face: MascotFace;
  visemeCode: string;
  setFace: (face: MascotFace) => void;
  setVisemeCode: (code: string) => void;
  holdThenIdle: (ackFace: MascotFace, ms?: number) => void;
  notifyListening: () => void;    // 新增：用户输入通知，2s 防抖回 idle
}
```

`notifyListening` 实现：
- 仅在 `face === 'idle'` 时切换到 `listening`（不打断 thinking/speaking 等活跃状态）
- 2s 防抖计时器，停止输入后回到 `idle`
- 发送消息 / 开始 streaming 时自动清除 listening 计时器

### 2.6 Mascot 显示位置

| 位置 | 组件 | 当前 | I5 |
|------|------|------|-----|
| Orb 悬浮球内 | `OrbBall.tsx` | ✅ `<RiveMascot face={mascotFace} size={ORB_HITBOX_SIZE} idlePoseRotation />` | 不变 |
| 发送按钮 | `MessageInput.tsx` | ✅ `<RiveMascot face={mascotFace} size={44} />` | 不变 |

> Orb 对话面板顶部无空间放置 Mascot，不在此处添加。

## 3. 不做的事（I5 范围外）

- ❌ jiwen 情感引擎集成（5 轴模型 + 主动行为），用户明确排除
- ❌ TTS 口型同步（需 TTS 引擎，后续迭代）
- ❌ 自定义 .riv 文件上传 UI（buffer 属性预留接口，UI 在设置页后续迭代）
- ❌ mascotPalette 换色系统（primaryColor/secondaryColor 属性预留，palette UI 后续迭代）
- ❌ Mascot 在 Orb 面板顶部显示（面板顶部无空间）

## 4. 参考项目

| 功能点 | 参考项目 | 参考文件 | 借鉴内容 |
|--------|---------|---------|---------|
| Rive 颜色换肤 | OpenHuman | RiveMascot.tsx L48-49 | `useViewModelInstanceColor('primaryColor', vmInstance)` + `useViewModelInstanceColor('secondaryColor', vmInstance)` |
| Rive buffer 加载 | OpenHuman | RiveMascot.tsx L55-56 | `buffer ? { buffer } : { src }` 优先级，避免双加载竞争 |
| 18 种 MascotFace | OpenHuman | Ghosty.tsx L20-38 | 完整表情词汇表（sleep/idle/listening/thinking/confused/speaking/happy/concerned/curious/proud/cautious/celebrating/writing/reading/waving/dancing/drinking_coffee/drinking_boba/normal） |
| FACE_TO_POSE 完整映射 | OpenHuman | riveMaps.ts L48-67 | 18→9 多对一映射表 |
| 口型系统 | OpenHuman | visemes.ts | 8 种 VisemeId（REST/A/E/I/O/U/M/F）+ `{openness, width}` + `lerpViseme` 插值 |
| 空闲动画轮转 | OpenHuman | RiveMascot.tsx L30-35 | `AMBIENT_IDLE_MIN_MS=6000, MAX=12000`，idle 6-12s 后随机切换 ambient pose |
| 宠物情绪映射 | OpenBitFun | agentCompanionMood.ts | `deriveAgentCompanionMood`：4 种 mood（rest/analyzing/waiting/working），从 `SessionStateMachine` 派生 |
| 情绪外观 facet | OpenBitFun | AgentCompanionPet.appearance.ts | `data-openbitfun-mood` 属性驱动 CSS：rest/analyzing/waiting/working/hover/dragging |

## 5. 验收标准

1. `@rive-app/react-webgl2` 正常加载 `mascot.riv`，悬浮球内可见 Mascot 动画
2. MascotFace 枚举包含 18 种表情 + `normal` legacy 别名
3. FACE_TO_POSE 映射覆盖全部 18 种 MascotFace → 9 种 RivePose
4. RiveMascot 支持 `primaryColor` / `secondaryColor` 属性驱动 Rive 颜色
5. RiveMascot 支持 `buffer` 属性从内存加载 .riv
6. 对话时 Mascot 表情实时变化：idle → listening → thinking → speaking → happy/concerned
7. 用户输入时 Mascot 切换到 `listening` 表情，停止输入 2s 后回 `idle`
8. 流式输出时 Mascot 口型随文字变化（viseme 联动）
9. 流式输出结束后根据回复内容判断 ack 表情（happy / proud / concerned）
10. 错误时 Mascot 显示 `concerned` 表情，700ms 后回 `idle`
11. 悬浮球内 Mascot 空闲时自动轮转 ambient pose（6~12s 间隔）
12. 发送按钮 Mascot 表情与悬浮球同步
13. `cargo check` ✅ / `tsc --noEmit` ✅ / `lint 0 errors` ✅
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
| MascotFace 新增 proud | 缺 `proud` 表情（工具调用成功 → celebration pose），其余 OpenHuman 扩展表情（sleep/confused/cautious/celebrating/writing/reading/dancing/drinking_coffee/drinking_boba/normal）语义与现有 face 重复或仅 ambient 用途，暂不引入 | — |
| FACE_TO_POSE 新增 proud 映射 | `proud` → `celebration` | OpenHuman riveMaps.ts L48-67 |
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
- `buffer` 切换时需配合 React `key` 强制重挂载：`<RiveMascot key={mascotId} buffer={...} />`，否则两个 .riv 运行时实例竞争同一个 DOM 容器
- `useViewModelInstanceColor('primaryColor', vmInstance)` + `useViewModelInstanceColor('secondaryColor', vmInstance)` 驱动 Rive 颜色属性
- `primaryColor` / `secondaryColor` 变更时 `useEffect` 调用 `setPrimaryColor` / `setSecondaryColor`

### 2.2 MascotFace 扩展

从 9 种扩展至 10 种，仅新增 `proud`（工具调用成功 → celebration pose）：

```typescript
export type MascotFace =
  | 'idle'
  | 'listening'
  | 'thinking'
  | 'speaking'
  | 'happy'
  | 'concerned'
  | 'curious'
  | 'working'
  | 'waving'
  | 'proud';      // 新增：工具调用成功
```

> **不引入的 OpenHuman 扩展表情及理由**：
> - `sleep` / `confused` / `cautious`：与 idle/thinking/concerned 语义重叠，无独立触发场景
> - `celebrating`：与 proud/happy 重叠
> - `writing` / `reading`：已有 working/curious 覆盖相同 RivePose
> - `dancing` / `drinking_coffee` / `drinking_boba`：仅 ambient pose 用途，空闲轮转已覆盖
> - `normal`：legacy alias，无外部调用方需要

### 2.3 FACE_TO_POSE 完整映射

```typescript
export const FACE_TO_POSE: Record<MascotFace, RivePose> = {
  idle:            'idle',
  listening:       'idle',
  thinking:        'thinking',
  speaking:        'idle',
  happy:           'celebration',
  concerned:       'thinking',
  curious:         'bookreading',
  working:         'writing',
  waving:          'hand_wave',
  proud:           'celebration',
};
```

> 10 种 face 映射到 9 种 RivePose（happy 和 proud 共享 celebration 动画），face 语义精简且每个都有明确交互触发。

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

- MessageInput 的 `onChange` 事件 → `useMascotStore.getState().notifyListening()`
- 2s 防抖：停止输入后自动回到 `idle`（仅在非 streaming 时）
- 发送消息时立即切换到 `thinking`（等待首 token），覆盖 listening

**working 状态实现**：

- Agent 工具调用期间（`isStreaming && messages 末尾含 tool 角色）→ `setFace('working')`
- 工具调用结束（流式输出继续）→ `setFace('speaking')`，由 useMascotChatSync 自动切换
- working 与 thinking 的区分：thinking = reasoning 阶段（思考链），working = 工具执行阶段（action）

#### 2.4.2 deriveMascotFaceFromChat 扩展

```typescript
export function deriveMascotFaceFromChat(
  isStreaming: boolean,
  isReasoningStreaming: boolean,
  hasError: boolean,
  isListening?: boolean,    // 新增
  isWorking?: boolean,      // 新增：Agent 工具调用中
): MascotFace {
  if (hasError) return 'concerned';
  if (isReasoningStreaming) return 'thinking';
  if (isWorking) return 'working';
  if (isStreaming) return 'speaking';
  if (isListening) return 'listening';
  return 'idle';
}
```

> 优先级：error > reasoning > working > streaming > listening > idle。reasoning 优先于 working，因为思考链通常先于工具调用。

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

> **hadToolCall 数据源**：从 chatStore.messages 中检测——当前对话轮次中是否存在 `role === 'tool'` 的消息。具体判断方式：`messages.slice(lastUserMsgIndex).some(m => m.role === 'tool')`。

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
| 10 种 MascotFace | — | — | 在现有 9 种基础上仅新增 `proud`（工具调用成功），其余 OpenHuman 扩展表情语义重复暂不引入 |
| FACE_TO_POSE 映射 | OpenHuman | riveMaps.ts L48-67 | 10→9 映射表，happy/proud 共享 celebration pose |
| 口型系统 | OpenHuman | visemes.ts | 8 种 VisemeId（REST/A/E/I/O/U/M/F）+ `{openness, width}` + `lerpViseme` 插值 |
| 空闲动画轮转 | OpenHuman | RiveMascot.tsx L30-35 | `AMBIENT_IDLE_MIN_MS=6000, MAX=12000`，idle 6-12s 后随机切换 ambient pose |
| 宠物情绪映射 | OpenBitFun | agentCompanionMood.ts | `deriveAgentCompanionMood`：4 种 mood（rest/analyzing/waiting/working），从 `SessionStateMachine` 派生 |
| 情绪外观 facet | OpenBitFun | AgentCompanionPet.appearance.ts | `data-openbitfun-mood` 属性驱动 CSS：rest/analyzing/waiting/working/hover/dragging |

## 5. 验收标准

1. `@rive-app/react-webgl2` 正常加载 `mascot.riv`，悬浮球内可见 Mascot 动画
2. MascotFace 枚举包含 10 种表情（idle/listening/thinking/speaking/happy/concerned/curious/working/waving/proud）
3. FACE_TO_POSE 映射覆盖全部 10 种 MascotFace → 9 种 RivePose
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
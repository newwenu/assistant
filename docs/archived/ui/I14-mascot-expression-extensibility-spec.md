# I14 — Mascot 表情映射可扩展化 规格说明

## 1. 交付目标

将表情驱动链路中三处硬编码映射重构为**数据驱动 + 声明式**架构，使新增表情、自定义语义规则、接入情感引擎均**无需改动编排层核心逻辑**。

> **前置依赖**：I5 ✅ + I5-Ext ✅（三层架构 + 注册表 + MascotFace 17 种 + useMascotChatSync）
> **核心动机**：I5-Ext 解决了渲染器层扩展性，但编排层（useMascotChatSync）的映射仍全量硬编码——状态边沿 if-else 链、语义正则规则、MascotFace 固定 union 三处封闭，无法在不改代码的前提下扩展。

## 2. 当前问题

| # | 瓶颈 | 位置 | 具体表现 |
|---|------|------|---------|
| P1 | 状态边沿映射硬编码 | `useMascotChatSync.ts` L49-65 | if-else 链写死 `justStartedStreaming → speaking` 等映射，无法新增触发源（如长时间闲置 → sleep）或调整优先级 |
| P2 | 语义规则硬编码 | `useMascotChatSync.ts` L6-27 | 6 组正则 + 优先级写死在闭包内，无法追加关键词/新规则/用户自定义 |
| P3 | MascotFace 固定 union | `core/types.ts` L1-17 | 新增表情需改 types.ts + 所有渲染器映射表，四处联动 |
| P4 | 无情感引擎接口 | — | ARCHITECTURE.md §3.4 规划 jiwen 5 轴模型，但无代码接口预留 |

## 3. 解决方案

### 3.1 P1+P2：映射规则数据化

将 `useMascotChatSync` 内的两类映射抽为可组合的规则对象，hook 仅做规则求值 + 状态边沿检测。

**状态边沿映射** → `ChatFacePolicy`

```typescript
interface ChatFacePolicy {
  onStreamStart: { reasoning: MascotFace; toolCall: MascotFace; default: MascotFace };
  onStreamEnd: MascotFace;
  onReasoningEnd: { toolCall: MascotFace; default: MascotFace };
  onError: MascotFace;
}
```

**语义规则** → `AckRule[]`

```typescript
interface AckRule {
  pattern: RegExp;
  face: MascotFace;
  requiresToolCall?: boolean;
  priority?: number;
}
```

`pickAckFace` 改为遍历 `AckRule[]`（按 priority 降序），首个匹配即返回。默认规则集从现有 6 组正则平移。

**组合入口**：

```typescript
interface ExpressionPolicy {
  chatFace: ChatFacePolicy;
  ackRules: AckRule[];
}
```

`useMascotChatSync` 接受可选 `policy` prop，默认使用 `DEFAULT_EXPRESSION_POLICY`（与当前行为完全一致）。

### 3.2 P3：MascotFace 半开放化

保持 `MascotFace` 为 branded string 而非固定 union，渲染器声明自己支持的 face 集合：

```typescript
type MascotFace = string & { __brand: "MascotFace" };

// 渲染器元数据新增
interface RendererMeta {
  // ...现有字段
  supportedFaces?: MascotFace[];  // 未声明则视为支持所有
}
```

内置 17 种 face 保留为 `BUILTIN_FACES` 常量 + 类型辅助，保证现有代码的类型安全不退化。`MascotRenderer` 调度时，若 face 不在当前渲染器的 `supportedFaces` 中，fallback 到 `idle`。

**向后兼容**：现有 `MascotFace` 的字面量 union 用法全部合法（string superset），仅新增 face 不再需要改 types.ts。

### 3.3 P4：情感引擎接口预留

定义 `EmotionEngine` 协议，`mascotStore` 可订阅其输出：

```typescript
interface EmotionEngine {
  subscribe: (listener: (event: EmotionEvent) => void) => () => void;
}

interface EmotionEvent {
  face: MascotFace;
  holdMs?: number;
}
```

`mascotStore` 新增可选 `attachEmotionEngine(engine)` 方法，将引擎事件桥接到 `setFace` / `holdThenIdle`。无引擎时行为不变。

## 4. 改动范围

| 文件 | 改动 | 性质 |
|------|------|------|
| `core/types.ts` | `MascotFace` → branded string；`RendererMeta` 新增 `supportedFaces` | 重构 |
| `core/types.ts` | 新增 `ChatFacePolicy` / `AckRule` / `ExpressionPolicy` / `EmotionEngine` / `EmotionEvent` 接口 | 新增 |
| `core/expressionPolicy.ts` | 默认映射数据（`DEFAULT_ACK_RULES` / `DEFAULT_CHAT_FACE_POLICY` / `DEFAULT_EXPRESSION_POLICY`），与逻辑分离 | 新增 |
| `useMascotChatSync.ts` | 硬编码 → 引用 `DEFAULT_EXPRESSION_POLICY` + 规则求值逻辑，零硬编码映射 | 重构 |
| `MascotRenderer.tsx` | 新增 `supportedFaces` fallback 逻辑 | 增强 |
| `mascotStore.ts` | 新增 `attachEmotionEngine` / `detachEmotionEngine` | 新增 |
| 各渲染器 `index.ts` | `meta` 新增 `supportedFaces` 声明 | 补充 |

## 5. 不做的事

- ❌ 情感引擎实现（仅定义接口，jiwen 集成是后续迭代）
- ❌ 用户 UI 配置语义规则（仅数据化，UI 在设置页后续迭代）
- ❌ MascotFace 完全动态化（保留 `BUILTIN_FACES` 常量集合，保证类型提示和 IDE 补全）

## 6. 验收标准

1. `DEFAULT_EXPRESSION_POLICY` 行为与当前硬编码完全一致（零回归）
2. 新增 `MascotFace` 值无需改 `core/types.ts`
3. 渲染器声明 `supportedFaces` 后，不支持的 face 自动 fallback 到 `idle`
4. `useMascotChatSync` 接受自定义 `ExpressionPolicy`，语义规则可追加/覆盖
5. `mascotStore.attachEmotionEngine(engine)` 可桥接外部引擎事件
6. `tsc --noEmit` ✅ / `eslint 0 errors` ✅ / `cargo check` ✅
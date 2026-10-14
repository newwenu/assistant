# I14 — Mascot 表情映射可扩展化 任务清单

> 规格：[I14-mascot-expression-extensibility-spec.md](./I14-mascot-expression-extensibility-spec.md)
> 前置：I5 ✅ + I5-Ext ✅

---

## A — MascotFace 半开放化 → spec §3.2

- [ ] A1. `core/types.ts`：`MascotFace` 从固定 union 改为 branded string `string & { __brand: "MascotFace" }`
- [ ] A2. 新增 `asFace(s: string): MascotFace` 品牌化函数 + `BUILTIN_FACES` 常量数组（保留 17 种内置值）
- [ ] A3. `RendererMeta` 新增可选 `supportedFaces?: MascotFace[]`
- [ ] A4. `MascotRenderer.tsx`：face 不在 `supportedFaces` 时 fallback 到 `idle`
- [ ] A5. 各渲染器 `index.ts`：`meta` 声明 `supportedFaces`（Rive/SVG 声明 17 种，GIF 声明 `["idle"]`）
- [ ] A6. 全局搜索 `MascotFace` 字面量用法，确保 branded string 兼容

---

## B — 映射规则数据化 → spec §3.1

- [ ] B1. `core/types.ts`：新增 `ChatFacePolicy` / `AckRule` / `ExpressionPolicy` 接口
- [ ] B2. `useMascotChatSync.ts`：现有 6 组正则 + if-else 链提取为 `DEFAULT_EXPRESSION_POLICY`
- [ ] B3. `useMascotChatSync.ts`：`pickAckFace` 重写为遍历 `AckRule[]`（按 priority 降序，首个匹配返回）
- [ ] B4. `useMascotChatSync.ts`：hook 接受可选 `policy?: ExpressionPolicy` prop，默认 `DEFAULT_EXPRESSION_POLICY`
- [ ] B5. 验证 `DEFAULT_EXPRESSION_POLICY` 行为与重构前完全一致

---

## C — 情感引擎接口预留 → spec §3.3

- [ ] C1. `core/types.ts`：新增 `EmotionEngine` / `EmotionEvent` 接口
- [ ] C2. `mascotStore.ts`：新增 `attachEmotionEngine(engine)` / `detachEmotionEngine()`，桥接引擎事件到 `setFace` / `holdThenIdle`
- [ ] C3. 无引擎时行为不变（attach 前后零回归）

---

## D — 验证

- [ ] D1. `tsc --noEmit` 通过
- [ ] D2. `eslint` 0 errors
- [ ] D3. `cargo check` 通过
- [ ] D4. spec §6 验收标准 1~6 全部通过
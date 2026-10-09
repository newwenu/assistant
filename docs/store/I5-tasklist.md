# I5 — Mascot 形象 + 表情联动 任务清单

> 规格：[I5-spec.md](./I5-spec.md)
> 前置：I2 ✅ + I4 ✅（chatStore 流式输出 + OrbConf 外观配置已就绪）

---

## 阶段 A — MascotFace 扩展 + FACE_TO_POSE 映射

- [x] A1. `faceStates.ts` MascotFace 新增 `proud`（工具调用成功），扩展至 10 种。不引入 OpenHuman 其余扩展表情（sleep/confused/cautious/celebrating/writing/reading/dancing/drinking_coffee/drinking_boba/normal），语义与现有 face 重复或仅 ambient 用途
- [x] A2. `riveMaps.ts` FACE_TO_POSE 新增 `proud` → `celebration` 映射，10 种 MascotFace → 9 种 RivePose
- [x] A3. `riveMaps.ts` pickAmbientPose / pickVisemeCode / toRiveVisemeCode 无需改动（已兼容）

---

## 阶段 B — RiveMascot 组件增强

- [x] B1. RiveMascotProps 新增 `primaryColor?: number` / `secondaryColor?: number` 属性
- [x] B2. RiveMascot 内部 `useViewModelInstanceColor('primaryColor', vmInstance)` + `useViewModelInstanceColor('secondaryColor', vmInstance)` 驱动 Rive 颜色
- [x] B3. RiveMascotProps 新增 `buffer?: ArrayBuffer` 属性，`buffer` 优先于 `src` 传给 `useRive`
- [x] B4. primaryColor / secondaryColor 变更 `useEffect` 调用 `setPrimaryColor` / `setSecondaryColor`

---

## 阶段 C — mascotStore 扩展

- [x] C1. MascotState 新增 `notifyListening(): void` 方法
- [x] C2. `notifyListening` 实现：仅在 `face === 'idle'` 时切换到 `listening`，2s 防抖计时器停止输入后回 `idle`
- [x] C3. `setFace` / streaming 开始时自动清除 listening 防抖计时器
- [x] C4. `deriveMascotFaceFromChat` 新增 `isListening?: boolean` + `isWorking?: boolean` 参数，优先级 error > reasoning > working > streaming > listening > idle

---

## 阶段 D — useMascotChatSync 增强

- [x] D1. 流式输出结束 ack 表情增强：`pickAckFace` 新增 `hadToolCall` 参数，含工具调用+成功词 → `proud`；流式开始时检测 hadToolCall → `working`
- [x] D2. CONCERNED_RE / HAPPY_RE 正则保持不变（已覆盖关键词）

---

## 阶段 E — MessageInput listening 集成

- [x] E1. MessageInput `onChange` 事件调用 `useMascotStore.getState().notifyListening()`
- [x] E2. 发送消息时 `onSend` 回调前无需额外处理（streaming 开始后 useMascotChatSync 自动覆盖 listening）

---

## 阶段 F — 集成验证

### F1. 编译验证

- [x] `tsc --noEmit` 通过
- [x] `npm run lint` 0 errors

**依赖**：A1~A3, B1~B4, C1~C4, D1~D2, E1~E2

---

### F2. 功能验证

- [ ] 悬浮球内 Mascot 正常显示 Rive 动画（idle 状态）
- [ ] MascotFace 枚举包含 10 种（idle/listening/thinking/speaking/happy/concerned/curious/working/waving/proud）
- [ ] FACE_TO_POSE 映射覆盖全部 10 种 face
- [ ] RiveMascot `primaryColor` / `secondaryColor` 属性可驱动 Rive 颜色变化
- [ ] RiveMascot `buffer` 属性可从内存加载 .riv
- [ ] 用户输入时 Mascot 切换到 `listening` 表情
- [ ] 停止输入 2s 后 Mascot 回到 `idle`
- [ ] 发送消息后 Mascot 切换到 `thinking`（等待首 token）
- [ ] 流式输出中 Mascot 切换到 `speaking`，口型随文字变化
- [ ] 流式输出结束后 Mascot 显示 `happy`（普通回复）或 `proud`（含工具调用+成功）
- [ ] 错误时 Mascot 显示 `concerned`，700ms 后回 `idle`
- [ ] 悬浮球空闲时 Mascot 自动轮转 ambient pose（6~12s 间隔）
- [ ] 发送按钮 Mascot 表情与悬浮球同步

**依赖**：F1

---

### F3. 验收标准

- [ ] I5-spec §5 验收标准 1~13 项全部通过

**依赖**：F2
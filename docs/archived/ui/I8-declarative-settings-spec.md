# I8 — 声明式设置系统 规格说明

## 1. 交付目标

将设置系统从"每项设置改 8+ 文件"的手动同步模式，重构为**声明式注册表 + amend 增量合并**模式，使新增设置项降至 1-2 文件，并遵循与 I5-Ext 渲染器注册表一致的开放-封闭原则。

> **前置依赖**：I6 ✅（原子写入 + 前端旁路消除 + 副作用归入 Rust 端 + 校验）
> **参考**：ChatGPT `AppConf::amend` 增量合并、BongoCat `projection` 协议层、I5-Ext `registerRenderer` 注册表模式

## 2. 当前问题

| # | 问题 | 影响 |
|---|------|------|
| P1 | 新增一个设置项需改 8-12 文件（Rust struct → cmd → register → TS interface → event → listen → dispatch → UI） | 开发效率低、易遗漏 |
| P2 | 前端 TS interface 与 Rust struct 手动同步，字段名/类型不一致时编译不报错 | 运行时静默失败 |
| P3 | 副作用逻辑硬编码在 `config.rs` 的 `if prev.xxx != conf.xxx` 分支中，新增副作用需改 Rust 代码 | 不封闭 |
| P4 | 设置 UI 组件与设置项定义耦合，每个设置项需手写 `Form.Item` + 控件 | 重复劳动 |
| P5 | 第三方插件无法注册自己的设置项 | 不可扩展 |

## 3. 架构设计

### 3.1 三层架构

```
┌─────────────────────────────────────────────────────┐
│  Layer 2: UI（设置页面）                             │
│  SettingsGroup.tsx  SettingField.tsx                │
│  根据 definitions 自动渲染，支持 customRender 逃生舱 │
├─────────────────────────────────────────────────────┤
│  Layer 1: Registry（注册表 + 调度）                  │
│  definitions.ts  settingRegistry.ts                 │
│  sideEffectRegistry.ts  settingsDispatcher.ts       │
├─────────────────────────────────────────────────────┤
│  Layer 0: Core（Rust amend + 校验）                  │
│  patch_conf command  AppConf::amend  validate       │
└─────────────────────────────────────────────────────┘

依赖方向：Layer N → Layer N-1（严格单向）
```

### 3.2 与 I5-Ext 渲染器注册表对齐

| 维度 | I5-Ext 渲染器注册表 | I8 声明式设置注册表 |
|------|---------------------|---------------------|
| 注册入口 | `registerRenderer(entry)` | `defineSetting(def)` |
| 能力声明 | `RendererCapabilities` | `SettingDefinition.type` + `.sideEffect` |
| 自动适配 | UI 根据 capabilities 隐藏/显示 | UI 根据 type 自动选择控件 |
| 扩展方式 | 第三方调用 registerRenderer | 第三方调用 defineSetting |
| 开放-封闭 | 新渲染器不改 Core | 新设置项不改框架层 |

## 4. 各层详细规格

### 4.1 Layer 0: Rust amend + 校验

#### `patch_conf` 命令

替代现有的 `set_app_conf` / `set_orb_conf` / `set_mascot_conf` 三个全量命令，统一为一个增量合并命令：

```rust
#[command]
pub fn patch_conf(
    app: AppHandle,
    group: String,       // "app" | "orb" | "mascot"
    patch: serde_json::Value,
) -> Result<(), String> {
    match group.as_str() {
        "app" => {
            let mut conf = AppConf::load().map_err(|e| e.to_string())?;
            let prev = conf.clone();
            conf = conf.amend(patch).map_err(|e| e.to_string())?;
            conf.validate()?;
            conf.save().map_err(|e| e.to_string())?;
            apply_side_effects(&app, &prev, &conf);
        }
        "orb" => { /* 同理 */ }
        "mascot" => { /* 同理 */ }
        _ => return Err(format!("unknown group: {}", group)),
    }
    Ok(())
}
```

#### `AppConf::amend`

借鉴 ChatGPT 参考项目的增量合并模式：

```rust
impl AppConf {
    pub fn amend(self, patch: Value) -> Result<Self, serde_json::Error> {
        let val = serde_json::to_value(self)?;
        let mut config: BTreeMap<String, Value> = serde_json::from_value(val)?;
        let new_json: BTreeMap<String, Value> = serde_json::from_value(patch)?;
        for (k, v) in new_json {
            config.insert(k, v);
        }
        serde_json::from_str::<AppConf>(&serde_json::to_string_pretty(&config)?)
    }
}
```

**关键特性**：
- amend 后反序列化回强类型，类型不匹配时返回错误 → 编译时安全 + 运行时校验
- 未知 key 被 BTreeMap 忽略 → 前端传多余字段不报错
- 缺失 key 保留原值 → 增量 patch 语义

#### 副作用注册表（Rust 侧）

```rust
type SideEffectFn = fn(&AppHandle, &AppConf, &AppConf) -> Result<(), String>;

static SIDE_EFFECTS: &[(&str, SideEffectFn)] = &[
    ("launch_at_login", |app, prev, conf| {
        if prev.launch_at_login != conf.launch_at_login {
            apply_autostart(app, conf.launch_at_login)?;
        }
        Ok(())
    }),
    ("stay_on_top", |app, prev, conf| {
        if prev.stay_on_top != conf.stay_on_top {
            set_always_on_top(app, conf.stay_on_top)?;
        }
        Ok(())
    }),
    // ...
];

fn apply_side_effects(app: &AppHandle, prev: &AppConf, conf: &AppConf) {
    for (_, handler) in SIDE_EFFECTS.iter() {
        let _ = handler(app, prev, conf);
    }
}
```

### 4.2 Layer 1: 注册表 + 调度

#### `SettingDefinition` — 设置项声明

```typescript
interface SettingDefinition {
  key: string;                    // 字段名，与 Rust struct 字段对齐
  group: "app" | "orb" | "mascot";
  type: SettingType;             // "boolean" | "string" | "number" | "color" | "slider" | "select" | "keybinding" | string;
  default: unknown;
  label: string;
  description?: string;
  sideEffect?: string;           // 声明副作用 ID，框架自动执行
  visible?: (conf: any) => boolean;  // 声明式联动
  // 类型专属属性
  min?: number;                  // slider
  max?: number;                  // slider
  step?: number;                 // slider
  options?: { label: string; value: string }[];  // select
  customRender?: FC;             // 逃生舱口：自定义 UI 渲染
}
```

#### `defineSetting` — 注册函数

```typescript
const definitions: SettingDefinition[] = [];

export function defineSetting(def: SettingDefinition): SettingDefinition {
  definitions.push(def);
  return def;
}

export function getDefinitionsByGroup(group: string): SettingDefinition[] {
  return definitions.filter(d => d.group === group);
}
```

#### `registerSettingType` — 类型渲染器注册

```typescript
interface SettingTypeRenderer {
  component: FC<SettingFieldProps>;
  validate?: (value: unknown) => boolean;
  toRustValue?: (value: unknown) => unknown;
}

const typeRenderers = new Map<string, SettingTypeRenderer>();

export function registerSettingType(type: string, renderer: SettingTypeRenderer): void {
  typeRenderers.set(type, renderer);
}
```

内置类型渲染器：

| type | component | 说明 |
|------|-----------|------|
| `boolean` | `Switch` | 开关 |
| `string` | `Input` | 文本输入 |
| `number` | `InputNumber` | 数字输入 |
| `color` | `ColorPicker` | 颜色选择 |
| `slider` | `Slider` | 滑块（需 min/max/step） |
| `select` | `Select` | 下拉选择（需 options） |
| `keybinding` | `KeybindingInput` | 快捷键录制 |

#### `registerSideEffect` — 副作用处理器注册（前端侧）

用于前端侧副作用（如 Rive 颜色更新），Rust 侧副作用已在 Rust 注册表处理：

```typescript
const sideEffectHandlers = new Map<string, (prev: any, next: any) => void | Promise<void>>();

export function registerSideEffect(id: string, handler: SideEffectHandler): void {
  sideEffectHandlers.set(id, handler);
}
```

#### `settingsDispatcher.ts` 改造

```typescript
export async function dispatchConfPatch(group: string, patch: Record<string, unknown>) {
  await invoke("patch_conf", { group, patch });
  // 刷新 store
  const store = useSettingsStore.getState();
  if (group === "app") await store.loadAppConf();
  else if (group === "orb") await store.loadOrbConf();
  else if (group === "mascot") await store.loadMascotConf();
  // 广播到其他窗口
  await emitSettingEventToPeers(SettingEvent.ConfChanged, { group, patch });
}
```

### 4.3 Layer 2: UI 自动渲染

#### `SettingField` — 通用设置字段组件

```typescript
export function SettingField({ def, value, onChange }: SettingFieldProps) {
  if (def.customRender) return <def.customRender value={value} onChange={onChange} />;
  const renderer = typeRenderers.get(def.type);
  if (!renderer) return <Input value={value} onChange={onChange} />;
  return <renderer.component value={value} onChange={onChange} def={def} />;
}
```

#### `SettingsGroup` — 设置分组组件

```typescript
export function SettingsGroup({ group }: { group: string }) {
  const defs = getDefinitionsByGroup(group);
  const conf = useConfByGroup(group);
  return (
    <Form layout="vertical">
      {defs
        .filter(d => !d.visible || d.visible(conf))
        .map(d => (
          <Form.Item key={d.key} label={d.label} tooltip={d.description}>
            <SettingField def={d} value={conf[d.key]} onChange={(v) => dispatchConfPatch(group, { [d.key]: v })} />
          </Form.Item>
        ))}
    </Form>
  );
}
```

#### 与 I5-Ext 渲染器设置面板集成

```typescript
function RendererSettingsSection() {
  const { mascotConf } = useSettingsStore();
  const renderer = getRenderer(mascotConf.renderer_id);
  return renderer?.SettingsPanel ? <renderer.SettingsPanel /> : null;
}
```

## 5. 设置项声明示例

### 5.1 当前 AppConf 字段迁移

```typescript
defineSetting({ key: "theme",           group: "app", type: "select",   default: "system", label: "主题",   options: THEME_OPTIONS }),
defineSetting({ key: "language",        group: "app", type: "select",   default: "zh-CN",  label: "语言",   options: LANG_OPTIONS }),
defineSetting({ key: "launch_at_login", group: "app", type: "boolean",  default: false,    label: "开机自启", sideEffect: "autostart" }),
defineSetting({ key: "close_to_tray",   group: "app", type: "boolean",  default: true,     label: "关闭到托盘" }),
defineSetting({ key: "stay_on_top",     group: "app", type: "boolean",  default: false,    label: "窗口置顶", sideEffect: "always_on_top" }),
```

### 5.2 当前 OrbConf 字段迁移

```typescript
defineSetting({ key: "always_on_top",     group: "orb", type: "boolean", default: false, label: "悬浮窗置顶",   sideEffect: "orb_always_on_top" }),
defineSetting({ key: "click_through",     group: "orb", type: "boolean", default: false, label: "鼠标穿透",     sideEffect: "orb_click_through" }),
defineSetting({ key: "scale_percent",     group: "orb", type: "slider",  default: 100,  label: "缩放",         min: 10, max: 300, step: 5 }),
defineSetting({ key: "opacity_percent",   group: "orb", type: "slider",  default: 100,  label: "透明度",       min: 10, max: 100, step: 5 }),
defineSetting({ key: "corner_radius_percent", group: "orb", type: "slider", default: 50, label: "圆角",    min: 0, max: 100, step: 5 }),
defineSetting({ key: "hover_delay_ms",    group: "orb", type: "slider",  default: 300,  label: "悬停延迟(ms)", min: 0, max: 5000, step: 50 }),
```

### 5.3 当前 MascotConf 字段迁移

```typescript
defineSetting({ key: "primary_color",    group: "mascot", type: "color", default: null, label: "主色" }),
defineSetting({ key: "secondary_color",  group: "mascot", type: "color", default: null, label: "辅色" }),
```

### 5.4 新增设置项示例（1 条声明即可）

```typescript
defineSetting({ key: "show_notification", group: "app", type: "boolean", default: true, label: "显示通知" }),
```

## 6. 改动量对比

| 操作 | 当前模式 | 声明式方案 |
|------|:--------:|:----------:|
| 加 bool 设置项（无副作用） | 8 文件 | **1 文件**（definitions.ts） |
| 加 bool 设置项（有副作用） | 9 文件 | **2 文件**（definitions.ts + sideEffect 注册） |
| 加 color 设置项 | 8 文件 | **1 文件**（definitions.ts） |
| 加新类型设置项 | 8+ 文件 | **2 文件**（registerSettingType + definitions.ts） |
| 加新设置分组 | 10+ 文件 | **1 文件**（definitions.ts，group 自动创建） |
| 删除设置项 | 8 文件 | **1 文件**（definitions.ts） |
| 第三方注册设置项 | 不可能 | **1 文件**（插件内 defineSetting） |

## 7. 迁移策略

### Phase 1：Rust amend + patch_conf 命令

- `AppConf` / `OrbConf` / `MascotConf` 新增 `amend()` 方法
- 新增 `patch_conf` command，注册到 `invoke_handler`
- 副作用从 `if prev.xxx != conf.xxx` 提取为 `SIDE_EFFECTS` 注册表
- 保留旧 `set_*_conf` 命令作为兼容层，内部改为调用 `patch_conf` 全量 patch

### Phase 2：前端注册表 + definitions

- 新建 `definitions.ts`，迁移所有现有设置项为 `defineSetting` 声明
- 新建 `settingRegistry.ts`，实现 `registerSettingType` + 内置类型渲染器
- 新建 `sideEffectRegistry.ts`，实现 `registerSideEffect` + 前端副作用注册
- `settingsDispatcher.ts` 新增 `dispatchConfPatch` 函数

### Phase 3：UI 自动渲染

- 新建 `SettingField.tsx` + `SettingsGroup.tsx`
- `GeneralSettings.tsx` / `OrbBallSection.tsx` / `OrbPanelSection.tsx` / `MascotColorSection.tsx` 改为使用 `SettingsGroup`
- 保留 `customRender` 逃生舱口用于快捷键录制等复杂 UI

### Phase 4：清理

- 删除旧 `set_*_conf` 命令（Phase 1 兼容层）
- 删除各 UI 组件中的手写 `Form.Item`
- 更新测试

## 8. 取舍与边界

| 方面 | 决策 | 理由 |
|------|------|------|
| 类型安全 | amend 后反序列化回强类型校验 | 兼顾增量便利和运行时安全 |
| 复杂 UI | `customRender` 逃生舱口 | 快捷键录制、颜色选择等无法用标准控件表达 |
| 副作用幂等 | 副作用处理器必须幂等 | 多设置项声明同一副作用时执行顺序不确定 |
| Rust struct 保留 | 保持 3 个 struct 不合并为单一 AppConfig | 与 I6 原子写入/校验逻辑兼容，渐进迁移 |
| 并发安全 | Rust 侧 `patch_conf` 加 Mutex | 多窗口并发 patch 时保证 load-merge-save 原子性 |
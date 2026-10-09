# I6 — 设置系统优化 规格说明

## 1. 交付目标

对设置系统进行分层优化，解决写入安全、写入路径统一、副作用归一、前端旁路消除四个核心问题。

> **前置依赖**：I4（设置系统已就绪）
> **参考**：BongoCat `bongocat-config` crate 的原子写入 + 修订检测模式

## 2. 当前问题

| # | 问题 | 影响 | 优先级 |
|---|------|------|--------|
| P1 | `save()` 直接 `File::create` 覆写，崩溃时半写状态导致配置丢失 | 数据安全 | P0 |
| P2 | `settingsStore.saveAppConf` 绕过 Dispatcher，副作用不执行、事件不广播 | 多窗口不同步 | P0 |
| P3 | 副作用散落三处（Dispatcher / useOrbConfSync / settingsStore），AppConf 和 OrbConf 模式不一致 | 维护困难 | P1 |
| P4 | `reload_shortcuts` 在 Store 里额外调用，不在 Dispatcher 统一流程中 | 职责分散 | P1 |

## 3. 优化方案

### 3.1 Phase 1 — 原子写入（P0）

Rust 端 `save()` 改为原子写入：先写 `.tmp` 临时文件 → `sync_all` 落盘 → `rename` 替换。崩溃后要么是旧文件、要么是新文件，不存在半写状态。

**新增文件**：`assistant-config/src/atomic_write.rs`

**修改文件**：
- `app_conf.rs`：`save()` 调用 `atomic_write`
- `orb_conf.rs`：`save()` 调用 `atomic_write`
- `lib.rs`：新增 `pub mod atomic_write`
- `Cargo.toml`：无需新增依赖

**实现细节**：
```rust
pub fn atomic_write(path: &Path, data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension("tmp");
    {
        let mut file = File::create(&temp)?;
        file.write_all(data)?;
        file.sync_all()?;
    }
    fs::rename(&temp, path)?;
    Ok(())
}
```

`load()` 增加崩溃恢复：检测 `.tmp` 文件存在时尝试解析，有效则提升为正式配置，无效则删除。

### 3.2 Phase 2 — 消除前端旁路（P0）

删除 `settingsStore` 中的 `saveAppConf` / `saveOrbConf` 旁路方法，所有写入统一走 Dispatcher。

**修改文件**：
- `settingsStore.ts`：删除 `saveAppConf` / `saveOrbConf`，`updateKeybinding` / `resetKeybindings` 中 `reload_shortcuts` 移入 Dispatcher
- `settingsDispatcher.ts`：`dispatchAppConfChanged` 中增加 keybindings 变更时自动 `reload_shortcuts`
- 所有调用 `saveAppConf` / `saveOrbConf` 的组件改为调用 Dispatcher

### 3.3 Phase 3 — 副作用注册表（P1）

将副作用从 Dispatcher 硬编码改为按字段注册的模式，新增 `sideEffects.ts`。

**新增文件**：`src/settings/sideEffects.ts`

**修改文件**：
- `settingsDispatcher.ts`：`dispatchAppConfChanged` 中副作用改为 `runSideEffects(conf)`
- `settingsDispatcher.ts`：`dispatchOrbConfChanged` 中增加副作用执行（与 AppConf 对齐）
- `useOrbConfSync.ts`：简化为只做 CSS 变量应用，不再执行窗口 API 副作用

### 3.4 Phase 4 — 配置校验（P1）

Rust 端新增 `validate()` 方法，写入前校验配置值合法性。

**修改文件**：
- `app_conf.rs`：新增 `validate()` 方法
- `orb_conf.rs`：新增 `validate()` 方法
- `cmd.rs`：`set_app_conf` / `set_orb_conf` 写入前调用 `validate()`

## 4. 验收标准

1. 原子写入：`save()` 使用临时文件 + rename，崩溃后 `load()` 可恢复
2. 崩溃恢复：`.tmp` 文件有效时自动提升，无效时删除并回退默认值
3. 写入路径统一：不存在绕过 Dispatcher 的写入方法
4. 副作用注册表：新增配置项只需 `registerSideEffect`，无需改 Dispatcher
5. OrbConf 副作用在 Dispatcher 层执行，useOrbConfSync 只管 CSS
6. 配置校验：非法值（如 opacity_percent=0、scale_percent=0）被拒绝
7. 编译通过：`cargo check` + `tsc --noEmit` + `npm run lint` 0 errors
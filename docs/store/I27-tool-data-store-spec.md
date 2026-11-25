# I27 — 工具数据存储与可重定位目录规格

| 项 | 值 |
|---|---|
| 状态 | Draft（搁置：便签延期，仓内无 `tool_data` 代码；接入时按 §14 清单实施） |
| 日期 | 2026-11-01（2026-11-25 修订：目录改为 core 条目登记、去掉平台盘符判定、按当前代码校正路径与文件清单） |
| 关联 | I26-mini-mode-clock-spec（已归档，便签延期）；I61 存储管理器（条目登记与观测口径） |
| 影响范围 | 存储层、配置、设置页、IPC、i18n |

本文定义「工具数据」这一存储单元的边界、落盘格式、**混淆（非加密保护）**、目录可重定位流程与读写接口。**为什么这样定** 在每节的「理由」中给出，不重复上游分析。

---

## 1. 概述

现有存储只有一个业务单元：`assistant.db`，里面同时装着会话、消息、供应商、附件引用、工具收藏与使用历史。工具没有自己的存储。

本规格新增第二个存储单元 **工具数据（tool data）**，满足两条：

1. **独立成文件**，可与 `assistant.db` 分别管理、分别搬迁。
2. **目录可由用户指定**，主应用提供切换入口与数据迁移。

交付分三块：`ToolDataStore`（独立 SQLite 文件 + 通用 KV + 工具专用表，固定密钥混淆）、目录重定位（校验 → 迁移 → 切换 → 落盘 → 广播，失败可回滚）、设置页入口（当前位置 / 更改位置 / 恢复默认）。

---

## 2. 目标

- 工具数据与聊天数据物理隔离：删 / 搬 / 坏一个不影响另一个。
- 用户可把工具数据放到任意可写目录（含网络盘、映射盘），不做技术限制。
- 切换目录不丢数据；失败时停留在原目录且原数据完好。
- 工具以 `tool_id` 为命名空间读写，无法越界访问其它工具的数据。
- 目录不可达时主应用照常启动，聊天功能不受影响。
- 文件被通用 SQLite 工具直接打开时读不到内容（**混淆**，不承诺抵抗针对性攻击）。
- 用户**不需要输入任何密码**，也不需要额外操作。

## 3. 非目标

- 附件（`attachments/`）、头像素材（`mascot-assets/`）、vault（`security/`）一起搬：属聊天与安全域，不在本单元内。
- 多目录并存或按工具分目录：一个存储单元只有一个活动根目录。
- **网络盘上的可靠性**：UNC `\\` 路径、映射的远程盘允许选择，只给提示不拦截（§9.2）。不承诺这类介质上不出现 `database is locked` 或断连损坏。
- 云同步、版本历史、多设备并发写。
- **加密保护**：本单元只混淆，不加密。密钥固定在二进制内，任何人拿到文件即可解出内容；不抵抗同机同用户进程、不抵抗内存读取、不抵抗逆向。真实保护由 vault 承担，工具数据不在其信任域内。
- **密码 / 系统凭据库档位**：不引入 `keyring`，不提供「设密码才可打开」的路径。
- **按工具独立密钥**：一个存储单元一把钥；工具级隔离靠命名空间，不靠独立密钥。
- 二进制大文件存储（见 §12）、用户自定义单工具的存储位置。
- 把 `assistant.db` 也变成可重定位（改动面远超本次范围）。

---

## 4. 既有事实

| 事实 | 位置 |
|---|---|
| 主库路径由 `AppDirs::database()` 给出，启动时 `init_db_localized` 打开后 `manage(DbState(Arc<Mutex<Connection>>, db_path))` | `src-tauri/src/setup.rs`、`src-tauri/crates/assistant-storage/src/db.rs` |
| 主库未设 `journal_mode`，走 SQLite 默认 `DELETE`（非 WAL） | 同上 |
| 库迁移只允许 ADD COLUMN，禁止 DROP / RENAME / 数据重写 | `src-tauri/crates/assistant-storage/src/migrations.rs` |
| 受管目录的唯一拼串点是 `AppDirs`，条目来自 `registry::MANAGED_ROOTS`；**新增受管目录 = 加 `dir_name` 常量且加一条登记**，只做前者由 V19 单测判红 | `src-tauri/src/app_dirs/{dir_name,registry,roots}.rs` |
| 配置变更走 `patch_conf(group, patch)`：先落盘、再跑 `conf_side_effects`，侧效失败只记日志不回滚 | `src-tauri/src/cmd/config.rs`、`cmd/mod.rs` |
| 配置组 struct 与 `*ConfDto` 分处 `assistant-config` 与 `assistant-protocol`，手工映射在 `dto_bridge.rs`；前端类型由 `npm run contract:gen` 生成 | `src-tauri/crates/assistant-config/src/{app_conf,dto_bridge}.rs`、`crates/assistant-protocol/src/config.rs` |
| workspace `rusqlite` 以 `bundled-sqlcipher` 启用，`PRAGMA key` 可用；`hkdf` 已在 workspace 声明 | `src-tauri/Cargo.toml` |
| `assistant-storage` 已依赖 `sha2`、`hex`、`serde_json`、`tokio`，**未依赖 `hkdf`** | `src-tauri/crates/assistant-storage/Cargo.toml` |
| `keyring` 仅是 `assistant-vault` 的依赖，`--no-default-features` 构建下不在依赖图中 | `cargo tree --no-default-features -i keyring` 实测无匹配 |
| 目录选择器已可用：`tauri_plugin_dialog` 已注册，`open({ directory: true })` 已有先例 | `src-tauri/src/plugins.rs`、`src/settings/shared/renderers/AssetPickerField.tsx` |
| 存储页快照是 `entries + inventory` 结构，条目一律来自登记表 / 分区自述；新增一个存储项 = 新增一条条目，不加平铺字段 | `src-tauri/src/cmd/storage/`、`src/settings/pages/storage/` |

由上可直接推出的六条实现约束：

- 目录切换**不能**走 `patch_conf`：侧效失败时配置已落盘，会留下「配置指向新目录、实际仍用旧目录」的不一致。必须由专用命令自行编排顺序。
- 新库必须是**独立文件**，否则无法只搬工具数据。
- 新库沿用 ADD COLUMN 迁移纪律，与主库同规则。
- 混淆**只能整库**：`PRAGMA key` 作用于连接，无法只作用于部分表；字段级加密会让搜索与索引失效，且对「仅混淆」毫无必要。
- 混淆密钥**不能**来自 `keyring`：那会让它成为无条件依赖，破坏 `--no-default-features` 构建的干净性。
- 默认位置必须**登记进 `MANAGED_ROOTS`**，否则存储页不会观测它，V19 单测也会判红。

---

## 5. 不变式

违反任一条即为实现错误：

| # | 不变式 |
|---|---|
| 1 | 存储单元只有 `tool-data.db` 一个文件（§12 的 `files/` 为预留扩展）；迁移、删除、备份只处理它 |
| 2 | 一个进程内只有一个活动工具数据库连接，且其路径与 `AppConf.tool_data_dir` 解析结果一致 |
| 3 | 切换目录期间新到达的工具数据命令立即失败（`error.tool_data_busy`），不得排队等待 |
| 4 | 切换失败时活动连接仍指向原目录，且原数据完整可读 |
| 5 | 目录不可达时不回落默认目录；存储进入 `unavailable`，写入命令失败 |
| 6 | 工具只能读写自己 `tool_id` 命名空间的数据 |
| 7 | KV 值是合法 JSON，单值 ≤ 256 KiB，单工具 ≤ 200 键 |
| 8 | 不删除用户指定的外部目录；只允许删除应用自管目录（默认位置） |
| 9 | 密钥只来自二进制内常量，只以整库 `PRAGMA key` 打开，无任何交互、无字段级加密 |
| 10 | 网络盘等风险路径只提示不拦截，不得因路径形态拒绝用户选择 |

---

## 6. 存储单元定义

### 6.1 目录布局与登记

```
<root>/                  # 默认 AppDirs::tool_data()（data 基根下 tool-data）；可为用户指定
  tool-data.db           # 唯一文件：固定密钥混淆的 SQLite 库
```

存储单元只有 `tool-data.db` 一个文件。密钥编译进二进制，**没有密钥文件、没有旁路元数据**，因此不存在「拷了库忘了拷钥」这类失败模式。`<root>/files/<tool_id>/` 是 §12 预留的二进制扩展位，v1 不创建、不写入。

**登记**（默认位置必须走统一登记通道，不能只在代码里拼串）：

- `app_dirs/dir_name.rs` 加 `TOOL_DATA = "tool-data"`，并并入 `ALL`。
- `app_dirs/registry.rs` 加 `CoreEntry::ToolData`（id `E20`，续接现有 `E1–E6 / E12–E15 / E17–E19`），`MANAGED_ROOTS` 加一行：`scope: Data`、`segments: &[dir_name::TOOL_DATA]`、`coverage: SelfFilesOnly`、`owner: "ToolDataStore"`、`risk: Forbidden`、`inventory: false`；`EXPECTED_ENTRY_COUNT` 13 → 14。
- `app_dirs/roots.rs` 加 `pub fn tool_data(&self) -> PathBuf`，与 `indexes()` / `tool_scratch()` 同形。
- 新增枚举分支会让 `entries/decl.rs` 的 `static_for` 与 `entries/eligibility.rs` 的穷尽匹配编译报错，按报错补齐即可。

**理由（三个字段的取值）**：

- `coverage: SelfFilesOnly`：v1 只有一个文件；`files/` 子树在 §12 才出现，出现时再改 `RecursiveTree`。
- `inventory: false`：活动根可被用户指到 data 基根之外，登记路径（默认位置）与扫描路径不同源，减法必然产出假余量 / 假负数（与 `E12` / `E19` 同构）。登记仍参与首层分类，`tool-data` 目录标 `declared`。
- `risk: Forbidden` + `action: None`：工具数据是用户产出，不提供任何回收入口。

### 6.2 数据库内容

建库时一次性创建（`execute_batch` + `IF NOT EXISTS`）：

```sql
CREATE TABLE IF NOT EXISTS tool_kv (
    tool_id    TEXT NOT NULL,
    key        TEXT NOT NULL,
    value_json TEXT NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (tool_id, key)
);

CREATE TABLE IF NOT EXISTS note_entries (
    id         TEXT PRIMARY KEY,
    title      TEXT NOT NULL DEFAULT '',
    content    TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    deleted_at INTEGER
);

CREATE INDEX IF NOT EXISTS idx_note_entries_active
    ON note_entries(deleted_at, updated_at);

CREATE TABLE IF NOT EXISTS tool_data_meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```

`tool_kv` 不额外建 `tool_id` 索引：主键 `(tool_id, key)` 的前缀已覆盖「按工具列键」。`tool_data_meta` 存 `schema_version`，与主库同名机制对齐。

### 6.3 两层结构：通用 KV + 工具专用表

| 层 | 表 | 适用 |
|---|---|---|
| 通用 | `tool_kv` | 工具的少量配置 / 状态（`tool_id` + `key` 二元组） |
| 专用 | 如 `note_entries` | 需要列表、排序、搜索、逐行更新的工具 |

**为什么不让便签也走 KV**：便签要按 `updated_at` 排序、按关键字搜索、逐条软删。塞进 KV 就得把整份便签集序列化成一个值，每次保存重写全部内容，条数增长后写放大不可接受，也用不上索引。专用表因此必要，KV 只承担「工具自己的小状态」。

**为什么两者仍在同一个库文件**：搬迁单位是文件而不是表；同库才能保证一次迁移覆盖全部工具数据，且只需一条连接。

### 6.4 与主库的边界

| 数据 | 位置 | 理由 |
|---|---|---|
| 会话、消息、供应商、附件引用、压缩状态、`app_settings` | `assistant.db` | 聊天核心，随应用数据走 |
| `tool_favorites` / `tool_history` | `assistant.db` | 已存在且被侧栏 / 工具箱读取；它们是**工具目录的元数据**而非工具产出，与主库同生命周期更稳 |
| 工具业务数据（便签等）、工具 KV | `tool-data.db` | 本规格范围 |

### 6.5 混淆（非加密保护）

`tool-data.db` 以 SQLCipher 打开（`PRAGMA key`），密钥是**编译进二进制的固定常量**：

```rust
// src-tauri/crates/assistant-storage/src/tool_data/key.rs
const TOOL_DATA_KEY_SEED: &[u8] = b"<编译期固定盐值>";
// 实际 key = HKDF-SHA256(TOOL_DATA_KEY_SEED, info = "tool-data.db/v1") → 32 字节
```

打开时 `PRAGMA key = "x'<hex(hkdf 输出)>'"`，之后建表、索引、查询全部照常。

**这是混淆，不是保护**。必须与以下事实一致，不得对外表述为「已加密」：

| 性质 | 是否成立 |
|---|---|
| 通用 SQLite 工具（DB Browser、`sqlite3` CLI）直接打开读不到内容 | 成立 |
| 换个账号、换台机器打开同一文件仍读不到 | 成立（密钥在二进制里，与机器无关） |
| 拿到二进制即可解出内容 | **不成立**——密钥就是常量，反编译即可得 |
| 抵抗同机同用户进程读内存 | **不成立** |
| 抵抗整目录被拷走 | **不成立**（配合上面的逆向即可） |

**为什么用 HKDF 而不是直接写死 32 字节**：常量字面量会以明文出现在二进制中，被 `strings` 一扫即得。经 HKDF 派生后二进制里只有 seed——**成本几乎为零，但让「随手 grep」失效**。这只是提高一点门槛，不改变上面任何一条「不成立」。

**为什么不做 keyring / 密码档位**：`keyring` 目前只是 `assistant-vault` 的依赖，`--no-default-features` 下不在依赖图（§4）；一旦工具数据用它就变成无条件依赖，破坏 `vault-optional-build.md`（已归档）的门禁性质。密码档位还会让「打开便签」变成「先输密码」，与「工具是零成本功能」直接冲突。需要真保护的内容应进 vault。

**为什么整库而不是字段级**：`PRAGMA key` 作用于连接，无法只作用于部分表；字段级会让 §6.2 的 `LIKE` 搜索、`updated_at` 排序与索引全部失效，且对「仅混淆」目标毫无收益。

**设置页文案**：只显示「工具数据位于 X」，**不显示任何加密 / 保护档位**。可写「该文件无法被通用 SQLite 工具直接打开」，不写「安全存储」「已加密」。

---

## 7. 配置与目录解析

### 7.1 配置字段

三处同步加字段（顺序固定，缺一即编译或契约失败）：

1. `src-tauri/crates/assistant-config/src/app_conf.rs`：`#[serde(default)] pub tool_data_dir: Option<String>`，`AppConf::new()` 置 `None`。
2. `src-tauri/crates/assistant-protocol/src/config.rs`：`AppConfDto` 加 `pub tool_data_dir: Option<String>`（输出 DTO 的 `Option` 不加 `skip_serializing_if`，键恒在、值 `null`）。
3. `src-tauri/crates/assistant-config/src/dto_bridge.rs`：`From<AppConf> for AppConfDto` 补映射。

然后跑 `npm run contract:gen` 重新生成 `src/types/protocol.contract.ts`。`src/settings/core/types/confTypes.ts` 只是 `AppConfDto` 的类型别名，**不手改**。

`validate()` 只做格式校验，**不检查存在性**：

```rust
if let Some(dir) = &self.tool_data_dir {
    let p = std::path::Path::new(dir);
    if !p.is_absolute() { return Err("tool_data_dir must be an absolute path".to_string()); }
    if p.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return Err("tool_data_dir must not contain '..'".to_string());
    }
}
```

**理由**：目录可能暂时不可达（盘符不存在、权限变化）。若在 `validate()` 里检查可达性，`AppConf::load()` 会直接失败并拖垮启动（`setup.rs` 已有该调用）。

### 7.2 解析规则

```rust
root = conf.tool_data_dir.map(PathBuf::from)      // 用户指定
       .unwrap_or_else(|| dirs.tool_data())       // 默认（§6.1 登记位置）
```

存的是用户选择时的绝对路径字符串，**不存 canonical 形式**：目录暂不可达时 `canonicalize` 会失败，存 canonical 等于把「暂时不可达」变成「永久丢失配置」。canonical 只在打开成功时用于「是否同一目录」的比较。

### 7.3 常量

| 常量 | 位置 | 值 |
|---|---|---|
| `dir_name::TOOL_DATA` | `src-tauri/src/app_dirs/dir_name.rs` | `"tool-data"`（目录名，进 `ALL`） |
| `DB_FILE_NAME` | `assistant-storage/src/tool_data/mod.rs` | `"tool-data.db"` |
| `MAX_KV_VALUE_BYTES` / `MAX_KV_KEYS_PER_TOOL` / `MAX_KEY_CHARS` | 同上 | `256 * 1024` / `200` / `128` |

前端不需要这些常量：位置与占用由 `get_tool_data_location` 返回。

---

## 8. 运行时状态与并发

### 8.1 状态结构

```rust
// src-tauri/src/tool_data/mod.rs
pub struct ToolDataState {
    inner: RwLock<StoreInner>,
    switching: AtomicBool,
}
struct StoreInner {
    root: PathBuf,
    db: Option<assistant_storage::tool_data::ToolDataDb>,   // None = 打开失败或切换中
    unavailable_reason: Option<String>,
}
#[derive(Clone)]
pub struct ToolDataStore(pub Arc<ToolDataState>);
```

`Connection` 由 `assistant_storage::tool_data::ToolDataDb` 持有，SQL 全部留在库 crate（与 `attachment_store` / `db` 同分工）；应用层只持路径与状态。

### 8.2 访问协议

```rust
impl ToolDataStore {
    pub fn with_db<T>(&self, f: impl FnOnce(&ToolDataDb) -> Result<T, String>) -> Result<T, String> {
        if self.0.switching.load(Ordering::Acquire) {
            return Err(assistant_i18n::t("error.tool_data_busy"));
        }
        let guard = self.0.inner.read().map_err(|_| assistant_i18n::t("error.tool_data_busy"))?;
        let db = guard.db.as_ref().ok_or_else(|| assistant_i18n::t("error.tool_data_unavailable"))?;
        f(db)
    }
}
```

`switching` 置位先于获取写锁，确保切换期间的读命令**立刻失败**而不是在 `RwLock` 上排队（不变式 3）。切换耗时与数据量成正比，排队会让前端卡住数秒。

### 8.3 打开与初始化

`assistant_storage::tool_data::open_at(root: &Path) -> Result<ToolDataDb, String>`：

1. `create_dir_all(root)`。
2. `Connection::open(root.join(DB_FILE_NAME))`。
3. `PRAGMA key = "x'<hex(HKDF(TOOL_DATA_KEY_SEED))>'"`（§6.5）。
4. `PRAGMA journal_mode = DELETE`（显式设置，不依赖默认值）。
5. `PRAGMA synchronous = FULL`。
6. 试探解密：`SELECT count(*) FROM sqlite_master`，失败即 `error.tool_data_open_failed`。
7. 建表（§6.2），读写 `tool_data_meta.schema_version`。

`synchronous = FULL` 的理由：便签是用户手写内容，断电丢最后一条不可接受；工具数据量小，额外开销可忽略。`journal_mode` 保持 `DELETE` 而非 `WAL`：`DELETE` 下关闭连接即保证文件静止，迁移只需处理单文件；`WAL` 需先 checkpoint 才能拷贝，多一个失败点。

`PRAGMA key` 之后 SQLCipher 不立即校验，首次读写才真正解密，因此步骤 6 就是那次试探。密钥恒定，此步失败只可能是**库损坏或非本应用生成的文件**，与「密码错误」无关——错误键因此是 `open_failed` 而非 `key_unavailable`。

### 8.4 启动

`setup.rs` 在 `AppConf::load()`、`manage(DbState)` 之后：

```rust
handle.manage(crate::tool_data::init(&handle));
```

`init` 内部：解析根目录 → `open_at` → 成功则 `db = Some`，失败则 `db = None` 且记录 `unavailable_reason`，**不阻断启动**（不变式 5）。失败原因写 `log::warn`，并在设置页展示。

---

## 9. 目录重定位

### 9.1 命令与 DTO

```rust
#[command] pub fn get_tool_data_location(app: AppHandle) -> Result<ToolDataLocationDto, String>
#[command] pub fn set_tool_data_dir(
    app: AppHandle,
    dir: Option<String>,         // None = 恢复默认
    keep_old_copy: Option<bool>, // 默认 false
) -> Result<ToolDataLocationDto, String>
```

`ToolDataLocationDto` 放在 `src-tauri/crates/assistant-protocol/src/storage.rs`（与 `StorageBreakdownDto` 同文件、同 serde 约定）：

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolDataLocationDto {
    pub dir: String,                // 解析后的绝对路径
    pub custom: bool,               // conf.tool_data_dir.is_some()
    pub state: ToolDataStateDto,    // ready | unavailable
    pub db_bytes: u64,              // tool-data.db 文件大小
    pub warning: Option<String>,    // 风险提示键，None = 无提示
    pub error: Option<String>,      // state = unavailable 时的原因
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub enum ToolDataStateDto { Ready, Unavailable }
```

`state` 用封闭枚举而非字符串（前端可穷尽分支）；**不含任何加密 / 保护字段**（§6.5）。

**为什么不用 `patch_conf`**：`patch_conf` 是「先落盘配置、再跑侧效」，侧效失败不回滚（§4）。目录切换可能因目标不可写、拷贝失败而失败，走该路径就会留下配置与运行状态不一致。本命令自己编排顺序，配置只在切换成功后写入。

### 9.2 校验（`validate_target`）

按顺序执行，任一失败即返回对应错误键：

| # | 规则 | 错误键 |
|---|---|---|
| 1 | 非空、绝对路径、不含 `..` | `error.tool_data_invalid_path` |
| 2 | 若已存在，必须是目录而非文件 | `error.tool_data_not_a_directory` |
| 3 | 若不存在，父目录必须存在且可写 | `error.tool_data_parent_not_writable` |
| 4 | 目标可写：写 `.write-test` 后删除 | `error.tool_data_not_writable` |
| 5 | 与当前根相同（canonical 比较，失败则字符串比较）→ 视为重试，直接重开连接并返回 | — |
| 6 | 目标已含 `tool-data.db` 且不是当前根 → 拒绝，避免覆盖未知数据 | `error.tool_data_target_occupied` |
| 7 | 目标与当前根存在包含关系（一个在另一个内部）→ 拒绝，避免拷贝递归 | `error.tool_data_nested_path` |

**网络盘不拦截**（不变式 10）：`\\` 开头的 UNC 路径一律允许选择，只在前端给一次提示。

**风险提示只做纯字符串判定**：`dir.starts_with(r"\\")` → `warning = "settings.storage.tool_data.warn_network"`，其余 `None`。**不引入 `DRIVE_REMOTE` 等平台 API 判定**：映射盘、`subst` 盘、WebDAV 挂载的返回值不一致，硬判会误报本机共享，而提示本身不阻断，误报的收益为零。判定是纯函数，无 IO、无平台依赖，因此不会因判定失败而拒绝路径。

规则 7 的理由：把新目录设在旧目录内部（或反之）会让拷贝过程自我包含，必然损坏数据。

### 9.3 迁移流程

1. `switching = true`（先置位，后取写锁）。
2. 取写锁；`validate_target`（§9.2）。
3. 记录 `old_root` 与 `old_counts`（`note_entries` / `tool_kv` 行数）。
4. 关闭旧连接（drop `db`）——`DELETE` 模式下关闭即保证文件静止。
5. 原为 `unavailable`（`db == None`）→ 跳过 6~8，直接到 9。
6. 拷贝 `old_root/tool-data.db` → `target/tool-data.db`（规则 6 已排除目标已有库）。
7. `open_at(target)`，校验 `PRAGMA integrity_check == "ok"` 且两张表行数与 `old_counts` 相等。
8. 校验失败 → 删本次新建的目标副本 → 重开 `old_root` → 返回错误。
9. 更新 `inner{ root, db }` → 写 `AppConf.tool_data_dir` 并保存。
10. 若 `!keep_old_copy` 且 `old_root` 是默认位置 → 删除该目录；外部旧目录一律不删（不变式 8）。
11. 释放写锁后清除 `switching`。
12. emit `tool-data:location-changed`，载荷 `ToolDataLocationDto`。

要点：

- **先关后拷**：`DELETE` 模式下没有待落盘的 `-wal`，关闭连接即代表文件静止。
- **单文件即可搬完**：密钥在二进制内，不随目录走，不存在「成对拷贝」的失败模式（不变式 1）。
- **校验行数不校验字节**：字节受 SQLite 页布局影响，行数等价性才是「数据都在」的可判定条件。
- **回滚顺序**：先恢复旧连接再清理目标副本；反过来会先删掉唯一成功副本，顺序错误即为实现错误。
- **外部旧目录不删**：用户选的目录可能还有别的东西。DTO 只返回新 `dir`，旧路径由前端在成功提示里告知「原位置数据保留在 X，可自行删除」。

### 9.4 恢复默认

`set_tool_data_dir(None, ...)`：

- 默认根 = `dirs.tool_data()`。
- 默认根不存在 → 直接创建并切换，无拷贝。
- 默认根存在且含 `tool-data.db` → 按 §9.2 规则 6 本应拒绝，此处是**例外**：默认根是应用自管位置，改为「以当前活动数据为准拷贝过去，覆盖前先把已存在的库重命名为 `tool-data.db.prev`」。该例外只对默认根生效，用户目录仍遵守规则 6。

### 9.5 前端

`src/services/desktop/toolDataService.ts`（与 `storageService` 同目录、同 `tauriInvoke` 封装）：

```ts
export const toolDataService = {
  getLocation: (): Promise<ToolDataLocationDto> => invoke("get_tool_data_location"),
  setDir: (dir: string | null, keepOldCopy = false): Promise<ToolDataLocationDto> =>
    invoke("set_tool_data_dir", { dir, keepOldCopy }),
  get: <T>(toolId: string, key: string): Promise<T | null> => invoke("tool_kv_get", { toolId, key }),
  set: (toolId: string, key: string, value: unknown): Promise<void> =>
    invoke("tool_kv_set", { toolId, key, value }),
  remove: (toolId: string, key: string): Promise<void> => invoke("tool_kv_delete", { toolId, key }),
  keys: (toolId: string): Promise<string[]> => invoke("tool_kv_list", { toolId }),
};
```

设置页在 `src/settings/pages/storage/` 新增 `ToolDataLocationBlock.tsx` 与 `useToolDataLocation.ts`（事件名常量按现有写法定义在本 hook 内，与 `useStorageSnapshot` 的 `STORAGE_INVENTORY_UPDATED` 同形，不往 `UiEvent` 枚举里加），并在 `StorageContent.tsx` 里渲染在 `AttachmentBlock` 之上：

| 元素 | 行为 |
|---|---|
| 当前位置 | 只读文本 + 复制按钮；`state === "unavailable"` 时显示红色原因 |
| 来源标记 | `custom` 为真显示「自定义」，否则「默认」 |
| 占用 | `dbBytes` 经 `src/utils/formatBytes.ts` 格式化 |
| 更改位置 | `open({ directory: true, multiple: false })` → 二次确认弹窗（迁移说明 + 「保留原位置副本」勾选 + `warning` 普通提示色）→ `setDir` |
| 恢复默认位置 | 确认弹窗 → `setDir(null)` |
| 切换中 | 按钮 loading，禁用重复点击；收到 `tool-data:location-changed` 后重拉 |

**不展示任何加密 / 保护状态**（§6.5）。设置页与工具页是互斥路由，不做跨页 flush。

---

## 10. 通用 KV 接口

### 10.1 命令

```rust
#[command] pub fn tool_kv_get(app, tool_id: String, key: String) -> Result<Option<serde_json::Value>, String>
#[command] pub fn tool_kv_set(app, tool_id: String, key: String, value: serde_json::Value) -> Result<(), String>
#[command] pub fn tool_kv_delete(app, tool_id: String, key: String) -> Result<(), String>
#[command] pub fn tool_kv_list(app, tool_id: String) -> Result<Vec<String>, String>
```

### 10.2 约束

| 约束 | 处理 |
|---|---|
| `tool_id` 为空或含非 `[a-z0-9-]` 字符 | 拒绝，`error.tool_data_invalid_tool_id` |
| `key` 长度 > `MAX_KEY_CHARS` | 拒绝，`error.tool_data_key_too_long` |
| 序列化后字节 > `MAX_KV_VALUE_BYTES` | 拒绝，`error.tool_data_value_too_large` |
| 该 `tool_id` 键数已达 `MAX_KV_KEYS_PER_TOOL` 且本次为新增 | 拒绝，`error.tool_data_too_many_keys` |
| 写入 | `INSERT ... ON CONFLICT(tool_id, key) DO UPDATE`，同时更新 `updated_at` |

`tool_id` 白名单字符的理由：它是命名空间主键，也是未来 `files/<tool_id>/` 的目录名，必须能安全地当路径分量。

### 10.3 工具侧用法

工具代码直接调 `toolDataService`，不额外加 hook；KV 只用于「工具自己的小状态」这类可丢失数据，需要列表 / 排序 / 搜索的内容走专用表（§6.3）。

---

## 11. 与便签（I26）的边界

I26 的 `note_entries` 建在 `tool-data.db`，不在 `assistant.db`：

| 层 | 内容 | 位置 |
|---|---|---|
| 本规格 | 存储单元、目录解析、连接与切换、KV | `tool-data.db` 与 `ToolDataStore` |
| 便签（延期） | `note_entries` 表结构、便签命令、前端界面 | 建表语句并入 §6.2 |

理由：便签是用户产出内容，最可能需要单独搬；便签表损坏不应牵连会话数据；便签**不做加密**，需要保密的内容进 vault（§3）。

`note_entries` 在本规格里只作为**专用表示例**：I26 已归档且便签延期，本规格不承诺便签命令的字段级契约；两份规格均未实现，因此不写数据迁移脚本。

---

## 12. 扩展点（v1 不实现）

| 扩展 | 预留 |
|---|---|
| 二进制文件 | `<root>/files/<tool_id>/` 目录约定已在 §6.1 声明；新增 `tool_file_*` 命令即可，不动既有命令 |
| 工具专用表 | 在 `tool_data::schema` 追加 `CREATE TABLE IF NOT EXISTS`，沿用 ADD COLUMN 迁移纪律 |
| 按工具统计占用 | `tool_data_meta` 已可承载；页面在 `E20` 行内展开，不改 DTO |

---

## 13. 存储管理器接入与其它改动

### 13.1 条目 `E20`（工具数据）

| 项 | 值 |
|---|---|
| 观测 | 走 `ToolDataStore` 的活动根 `file_stats`（**不读 `dirs.tool_data()`**，否则自定义后显示的是默认位置） |
| `availability` | 文件不存在 → `missing`；打开失败 / 切换中 → `unavailable`（区分「没有」与「取不到」） |
| 回收 | `action: none`、`eligibility: notEligible`、`reason: userData`、`risk: forbidden`，不提供清理入口 |
| 对账 | `inventory: false`（§6.1） |

`get_storage_breakdown` 的 DTO **不加平铺字段**：工具数据就是一条普通条目，`entries` 里天然出现。

### 13.2 其它改动

| 项 | 改动 |
|---|---|
| 事件 | `protocol/events.rs` 的 `event_name` 加 `TOOL_DATA_LOCATION_CHANGED = "tool-data:location-changed"`（广播，只通知「可重拉」） |
| IPC | `ipc.rs` 注册 6 个命令 |
| i18n | `settings.storage.tool_data.*`（标题、位置、默认、自定义、更改、恢复默认、迁移确认、成功、原位置保留、`warn_network`）写入 `src/i18n/locales/{zh-CN,en-US}.json`；`error.tool_data_*`（`busy` / `unavailable` / `open_failed` / `invalid_path` / `not_a_directory` / `parent_not_writable` / `not_writable` / `target_occupied` / `nested_path` / `invalid_tool_id` / `key_too_long` / `value_too_large` / `too_many_keys`）写入 `src-tauri/crates/assistant-i18n/locales/{zh-CN,en-US}.json`，两套 locale 键集一致 |

---

## 14. 文件清单

新增：

| 文件 | 内容 |
|---|---|
| `src-tauri/crates/assistant-storage/src/tool_data/mod.rs` | `open_at`、建库、meta、常量、`ToolDataDb` |
| `src-tauri/crates/assistant-storage/src/tool_data/kv.rs` | `tool_kv` 的 CRUD 与限额校验 |
| `src-tauri/crates/assistant-storage/src/tool_data/key.rs` | HKDF 派生（常量 seed） |
| `src-tauri/src/tool_data/mod.rs` | 根目录解析、`ToolDataStore` 状态、切换与迁移编排、`init` |
| `src-tauri/src/cmd/tool_data/mod.rs` | 6 个命令 |
| `src/services/desktop/toolDataService.ts` | 前端命令代理 |
| `src/settings/pages/storage/ToolDataLocationBlock.tsx` | 位置分组 |
| `src/settings/pages/storage/useToolDataLocation.ts` | 取数 + 事件订阅 |

修改：

| 文件 | 改动 |
|---|---|
| `src-tauri/src/app_dirs/dir_name.rs` | `TOOL_DATA` 常量 + 并入 `ALL` |
| `src-tauri/src/app_dirs/registry.rs` | `CoreEntry::ToolData`（`E20`）+ `MANAGED_ROOTS` 一行 + `EXPECTED_ENTRY_COUNT` 14 |
| `src-tauri/src/app_dirs/roots.rs` | `tool_data()` |
| `src-tauri/src/cmd/storage/entries/{decl,eligibility}.rs` | `E20` 静态档（编译器点名补齐） |
| `src-tauri/src/cmd/storage/sample/{collect,rows}.rs` | `E20` 采样（走活动根） |
| `src-tauri/crates/assistant-config/src/app_conf.rs` | `tool_data_dir` 字段 + `validate` |
| `src-tauri/crates/assistant-config/src/dto_bridge.rs` | `AppConf → AppConfDto` 补映射 |
| `src-tauri/crates/assistant-protocol/src/config.rs` | `AppConfDto.tool_data_dir` |
| `src-tauri/crates/assistant-protocol/src/storage.rs` | `ToolDataLocationDto` + `ToolDataStateDto` |
| `src-tauri/crates/assistant-storage/Cargo.toml` | 加 `hkdf = { workspace = true }`（workspace 已声明） |
| `src-tauri/src/setup.rs` | `tool_data::init` + `manage(ToolDataStore)` |
| `src-tauri/src/cmd/mod.rs` | 导出 `tool_data` |
| `src-tauri/src/ipc.rs` | 注册 6 个命令 |
| `src-tauri/src/protocol/events.rs` | `TOOL_DATA_LOCATION_CHANGED` |
| `src/settings/pages/storage/StorageContent.tsx` | 渲染位置分组 |
| `src/i18n/locales/{zh-CN,en-US}.json`、`src-tauri/crates/assistant-i18n/locales/{zh-CN,en-US}.json` | 键集 |
| `src/types/protocol.contract.ts` | 由 `npm run contract:gen` 生成，不手改 |

`Cargo.toml` 唯一新增依赖是 `hkdf`（`rusqlite` / `sha2` / `hex` / `serde_json` / `tokio` 均已在用）。**不引入 `keyring`**（§6.5）。

---

## 15. 验收

静态检查：

- [ ] `npm run contract:gen` 后 `npm run typecheck` / `npm run lint` / `npm run test` 通过
- [ ] `cargo fmt` 后 `cargo test`（cwd = `src-tauri`）通过，含 V19 登记表自检

单元测试（Rust，用 `tempfile`）：

- [ ] `validate_target`：相对路径、含 `..`、指向文件、父目录不可写、目标已有库、目标在当前根内部 —— 六类各返回对应错误
- [ ] `validate_target` 对 UNC 路径不报错，`warning` 有值
- [ ] 迁移：源含 N 条便签与 M 个 KV → 新目录行数一致；默认位置的源目录被删除；外部源目录不被删除
- [ ] 迁移失败注入（目标不可写）：活动连接仍指向原目录、原数据可读、目标无残留 `tool-data.db`
- [ ] `unavailable`：配置指向不存在的盘 → `init` 成功但 `db = None`；KV 命令返回 `error.tool_data_unavailable`
- [ ] `set_tool_data_dir(Some(同一路径))` 在 `unavailable` 下能重开连接（重试语义）
- [ ] 混淆：同一文件用通用 SQLite 打开失败；用派生 key 打开成功
- [ ] KV 限额：超 256 KiB、超 200 键、非法 `tool_id` 均拒绝
- [ ] 切换期间并发：切换进行中 `tool_kv_set` 立即返回 `error.tool_data_busy`，不阻塞

手工回归：

- [ ] 默认位置创建便签 → 改到另一本地盘 → 重启 → 数据仍在，路径显示新位置；存储页 `E20` 占用与 `get_tool_data_location` 的 `dbBytes` 一致
- [ ] DB Browser 打开 `tool-data.db`：提示「file is not a database」，读不到内容
- [ ] 目录被删除后启动：应用正常进入，聊天可用，位置分组显示不可用原因，`E20` 显示 `unavailable`
- [ ] 选择 UNC 路径：出现网络盘提示，确认后仍能切换成功（不被拦截）
- [ ] 恢复默认后原外部目录数据保留，提示文案指出旧路径

---

## 16. 未验证前提

1. **SQLite 在网络盘（SMB/NFS）上的可靠性**：`DELETE` + `synchronous = FULL` 能降低风险但不能消除锁语义差异。需实测在网络盘上编辑便签是否出现 `database is locked`，以决定提示文案强度。
2. **同步盘（OneDrive/Dropbox）的并发写冲突**：两设备同时写同一文件会产生冲突副本。v1 只记录该风险，不做检测。
3. **`open({ directory: true })` 在 Windows 上返回的路径形式**：可能是 `\\?\` 前缀或正斜杠。需实测后统一规范化，否则「同一路径」判断与 UNC 提示都会失效。
4. **HKDF 派生值在 `PRAGMA key` 中的编码**：`x'<hex>'` 传 32 字节原始 key 是标准写法，需确认 `bundled-sqlcipher` 构建下行为一致（vault 已有同类用法可对照）。
5. **非 UTF-8 路径**：`tool_data_dir` 存 `String`，极端路径无法表示。需确认是否有必要改为平台原生路径类型。
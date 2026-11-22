# I27 — 工具数据存储与可重定位目录规格

| 项 | 值 |
|---|---|
| 状态 | Draft |
| 日期 | 2026-11-01 |
| 关联 | I26-mini-mode-clock-spec（已归档；便签延期，本存储暂无消费者） |
| 影响范围 | 存储层、配置、设置页、IPC、i18n |

本文档定义「工具数据」这一存储单元的边界、落盘格式、**混淆（非加密保护）**、目录可重定位流程，以及工具的读写接口。**为什么这样定** 在每节的「理由」中给出，不重复上游分析。

---

## 1. 概述

现有存储只有一个单元：`app_data_dir/assistant.db`，里面同时装着会话、消息、供应商、附件引用、工具收藏与使用历史。工具没有自己的存储。

本规格新增第二个存储单元 **工具数据（tool data）**，满足两条：

1. **独立成文件**，可与 `assistant.db` 分别管理。
2. **目录可由用户指定**，主应用提供切换入口与数据迁移。

交付分三块：

- `ToolDataStore`：一个独立的 SQLite 文件 + 通用 KV 命名空间 + 工具专用表；库以固定密钥混淆，使通用 SQLite 工具无法直接读取。
- 目录重定位：校验 → 迁移 → 切换 → 落盘 → 广播，失败可回滚。
- 设置页入口：展示当前位置、更改位置、恢复默认。

---

## 2. 目标

- 工具数据与聊天数据物理隔离：删/搬/坏一个不影响另一个。
- 用户可把工具数据放到任意可写目录（含网络盘、映射盘），不做技术限制。
- 切换目录不丢数据；失败时停留在原目录且原数据完好。
- 工具以 `toolId` 为命名空间读写，无法越界访问其它工具的数据。
- 目录不可达时主应用照常启动，聊天功能不受影响。
- 文件被通用 SQLite 工具直接打开时读不到内容（**混淆**，不承诺抵抗针对性攻击）。
- 用户**不需要输入任何密码**，也不需要额外操作。

## 3. 非目标

- 附件（`attachments/`）、头像素材（`mascot-assets/`）、vault（`security/`）一起搬：它们属聊天与安全域，不在本单元内。
- 工具数据目录的**多目录并存**或按工具分目录：一个存储单元只有一个活动根目录。
- **网络盘上的可靠性**：UNC `\\` 路径、映射的远程盘允许选择，只给提示不拦截（见 §9.2.1）。不承诺这类介质上不出现 `database is locked` 或断连损坏。
- 工具数据的云同步、版本历史、多设备并发写。
- **加密保护**：本单元只混淆，不加密。密钥固定在二进制内，任何人拿到文件即可解出内容；不抵抗同机同用户进程、不抵抗内存读取、不抵抗逆向。真实保护由 vault 承担，工具数据不在其信任域内。
- **密码 / 系统凭据库档位**：不引入 `keyring`，不提供「设密码才可打开」的路径。需要真保护的用户应把敏感内容放进 vault，而不是工具数据。
- **按工具独立密钥**：一个存储单元一把钥；工具级隔离靠数据命名空间，不靠独立密钥。
- 二进制大文件存储（见 §12 扩展点）。
- 用户自定义单工具的存储位置。
- 把 `assistant.db` 也变成可重定位（改动面远超本次范围）。

---

## 4. 既有事实

| 事实 | 位置 |
|---|---|
| 主库路径固定为 `app_data_dir/assistant.db`，启动时 `init_db` 打开后 `manage(DbState(Mutex<Connection>))` | `src-tauri/src/setup.rs`、`src-tauri/crates/assistant-storage/src/db.rs` |
| 主库无 `journal_mode` 设置，走 SQLite 默认 `DELETE`（非 WAL） | `src-tauri/crates/assistant-storage/src/db.rs` |
| 主库迁移只允许 ADD COLUMN，禁止 DROP / RENAME / 数据重写 | `db.rs` 文件末尾 Migration pattern 注释 |
| 配置文件在 `dirs::config_dir()/config.json`，与 `app_data_dir` 是两个目录 | `src-tauri/crates/assistant-config/src/json_config.rs`、`app_conf.rs` |
| 配置变更走 `patch_conf(group, patch)` → 先保存 → 再执行 `CONF_APPLIERS` 侧效 | `src-tauri/src/cmd/config.rs`、`cmd/mod.rs` `conf_side_effects` |
| 侧效函数签名为 `fn(&AppHandle, Value) -> Result<(), String>`，失败只记日志，不回滚已保存的配置 | `cmd/mod.rs` `ConfApplier` |
| 附件存储为内容寻址目录 + `AttachmentStore::new(dir)`，目录在启动时固定 | `src-tauri/crates/assistant-storage/src/attachment_store.rs`、`setup.rs` |
| 存储设置页已有：附件占用明细、按会话分组、删除孤儿 | `src/settings/pages/StorageSettings.tsx` |
| `get_storage_breakdown` 汇总附件占用 | `src-tauri/src/cmd/storage.rs` |
| 目录选择器已可用：Rust 端已注册 `tauri_plugin_dialog`，前端已装 `@tauri-apps/plugin-dialog`，能力含 `dialog:default` | `src-tauri/src/plugins.rs`、`package.json`、`src-tauri/capabilities/default.json` |
| 已有调用 `open({ directory: true })` 的先例 | `src/settings/shared/renderers/AssetPickerField.tsx` |
| `assistant-storage` 已依赖 `rusqlite`、`uuid`、`thiserror`、`tokio` | `src-tauri/crates/assistant-storage/Cargo.toml` |
| workspace 的 `rusqlite` 以 `bundled-sqlcipher` 启用，`PRAGMA key` 可用 | `src-tauri/Cargo.toml` workspace 依赖 |
| `keyring = "4"` 仅是 `assistant-vault` 的依赖，`--no-default-features` 构建下**不在依赖图中** | `cargo tree --no-default-features -i keyring` 实测无匹配 |
| `assistant-crypto` 非可选，已含 `hkdf`、`sha2`、`zeroize` | `src-tauri/crates/assistant-crypto/Cargo.toml` |
| vault 已有「SQLCipher `PRAGMA key` + 密钥派生」的先例，可参考打开流程 | `src-tauri/crates/assistant-vault/src/security_db/mod.rs`、`src-tauri/crates/assistant-crypto/src/vault_crypto/mod.rs` |

由上可直接推出的五条实现约束：

- 目录切换**不能**走 `patch_conf`：侧效失败时配置已落盘，会留下「配置指向新目录、实际仍用旧目录」的不一致状态。必须由专用命令自行编排顺序。
- 新库必须是**独立文件**，否则无法只搬工具数据。
- 新库沿用 ADD COLUMN 迁移纪律，与主库同规则，避免出现无迁移脚本的破坏性变更。
- 混淆**只能整库**：SQLCipher 的 `PRAGMA key` 作用于连接，无法只作用于部分表。字段级加密会让搜索与索引失效，且对「仅混淆」而言毫无必要。
- 混淆密钥**不能**来自 `keyring`：那会让 `keyring` 成为无条件依赖，破坏 `--no-default-features` 构建的干净性（§3）。

---

## 5. 不变式

违反任一条即为实现错误：

| # | 不变式 |
|---|---|
| 1 | 工具数据只写入 `tool-data.db` 与其同级目录，**不得**写入 `assistant.db` |
| 2 | 一个进程内只有一个活动工具数据库连接，且其路径与 `AppConf.tool_data_dir` 解析结果一致 |
| 3 | 切换目录期间，新到达的工具数据命令立即失败（`error.tool_data_busy`），不得排队等待 |
| 4 | 切换失败时，活动连接必须仍指向原目录，且原数据完整可读 |
| 5 | 目录不可达时不回落默认目录；存储进入 `unavailable`，写入命令失败 |
| 6 | 工具只能读写自己 `tool_id` 命名空间的数据 |
| 7 | KV 值必须是合法 JSON，且单值 ≤ 256 KiB、单工具 ≤ 200 键 |
| 8 | 不删除用户指定的外部目录；只允许删除应用自管目录（默认位置） |
| 9 | 混淆密钥**只来自二进制内常量**，不得读取外部文件、系统凭据库或用户输入；打开库不得产生任何交互 |
| 10 | 工具数据只以整库 SQLCipher 打开，**不得**对任何表或字段做应用层加密 |
| 11 | 存储单元只有 `tool-data.db` 一个文件；迁移、删除、备份只处理它 |
| 12 | 网络盘等风险路径**只提示不拦截**，不得因路径形态拒绝用户选择 |

---

## 6. 存储单元定义

### 6.1 目录布局

```
<root>/                      # 默认 app_data_dir/tool-data；可为用户指定
  tool-data.db               # 唯一文件：固定密钥混淆的 SQLite 库
```

存储单元只有 `tool-data.db` 一个文件。混淆密钥编译进二进制，**没有密钥文件、没有旁路元数据**，因此不存在「拷了库忘了拷钥」这类失败模式。

`<root>/files/<tool_id>/` 是 §12 预留的二进制扩展位，v1 不创建、不写入。

**理由**：迁移单位是单文件，可退化为「拷贝一个文件 + 校验行数」，无需递归遍历、无需处理目录冲突、无需成对回滚。

### 6.2 数据库内容

`tool-data.db` 建库时一次性创建（`execute_batch` + `IF NOT EXISTS`）：

```sql
CREATE TABLE IF NOT EXISTS tool_kv (
    tool_id    TEXT NOT NULL,
    key        TEXT NOT NULL,
    value_json TEXT NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (tool_id, key)
);

CREATE INDEX IF NOT EXISTS idx_tool_kv_tool ON tool_kv(tool_id);

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

`tool_data_meta` 存 `schema_version`，与主库同名机制对齐。

### 6.3 两层结构：通用 KV + 工具专用表

| 层 | 表 | 适用 |
|---|---|---|
| 通用 | `tool_kv` | 工具的少量配置/状态（`tool_id` + `key` 二元组） |
| 专用 | 如 `note_entries` | 需要列表、排序、搜索、逐行更新的工具 |

**为什么不让便签也走 KV**：便签要按 `updated_at` 排序、按关键字搜索、逐条软删。塞进 KV 就得把整份便签集序列化成一个值，每次保存重写全部内容，条数增长后写放大不可接受，也无法用索引。因此专用表是必要的，KV 只承担「工具自己的小状态」。

**为什么两者仍在同一个库文件**：搬迁单位是目录/文件，不是表。同库才能保证一次迁移覆盖全部工具数据，且只需一条连接。

### 6.4 与主库的边界

| 数据 | 位置 | 理由 |
|---|---|---|
| 会话、消息、供应商、附件引用、压缩状态、`app_settings` | `assistant.db` | 聊天核心，随应用数据走 |
| `tool_favorites` / `tool_history` | `assistant.db` | 已存在且被侧栏/工具箱读取；搬走会让「收藏与历史」在目录不可达时失效，属刻意保留 |
| 工具业务数据（便签等）、工具 KV | `tool-data.db` | 本规格范围 |

`tool_favorites` / `tool_history` 不迁入，是有意决定：它们是**工具目录的元数据**而非工具产出，与主库同生命周期更稳。

### 6.5 混淆（非加密保护）

`tool-data.db` 以 SQLCipher 打开（`PRAGMA key`），但密钥是**编译进二进制的固定常量**：

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

**为什么用 HKDF 而不是直接写死 32 字节**：常量字面量会以明文出现在二进制中，被 `strings` 一扫即得。经 HKDF 派生后，二进制里只有 seed，派生逻辑在代码里——**成本几乎为零，但让「随手 grep」失效**。这只是提高一点门槛，不改变上述任何一条「不成立」。

**为什么不做 keyring / 密码档位**：
- `keyring` 目前只是 `assistant-vault` 的依赖，`--no-default-features` 构建下不在依赖图中（§4 实测）。一旦工具数据用它，`keyring` 变成**无条件依赖**，破坏 `vault-optional-build.md`（已归档）的门禁性质。
- 密码档位会让「打开便签」变成「先输密码」，与「工具是零成本功能」直接冲突。
- 需要真保护的内容应进 vault，工具数据不在其信任域内（§3）。

**为什么整库而不是字段级**：SQLCipher 的 `PRAGMA key` 作用于连接，无法只作用于部分表；字段级会让 §6.2 的 `LIKE` 搜索、`updated_at` 排序与索引全部失效，且对「仅混淆」目标毫无收益。

**设置页文案**：只显示「工具数据位于 X」，**不显示任何加密/保护档位**，也不显示「已加密」。可在帮助文本写「该文件无法被通用 SQLite 工具直接打开」，不写「安全存储」。

**需要真保护时的出口**：把内容放进 vault（`assistant-vault`），而不是期待工具数据加密。

---

## 7. 目录解析与配置

### 7.1 配置字段

`AppConf` 新增一个字段：

```rust
#[serde(default)]
pub tool_data_dir: Option<String>,   // None = 默认位置
```

`AppConf::new()` 置 `None`。TS `AppConf` 同步：

```ts
toolDataDir: string | null;
```

`validate()` 只做格式校验，**不检查存在性**：

```rust
if let Some(dir) = &self.tool_data_dir {
    let p = std::path::Path::new(dir);
    if !p.is_absolute() {
        return Err("tool_data_dir must be an absolute path".to_string());
    }
    if p.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return Err("tool_data_dir must not contain '..'".to_string());
    }
}
```

**理由**：目录可能暂时不可达（盘符不存在、权限变化）。若在 `validate()` 里检查可达性，配置加载会直接失败，进而拖垮启动时的 `AppConf::load()`（`setup.rs` 已有该调用）。

### 7.2 解析规则

```
root = conf.tool_data_dir.map(PathBuf::from)      // 用户指定
       .unwrap_or(app_data_dir.join("tool-data"))  // 默认
```

存的是用户选择时的绝对路径字符串，**不存 canonical 形式**：目录暂不可达时 `canonicalize` 会失败，存 canonical 等于把「暂时不可达」变成「永久丢失配置」。canonical 只在打开成功时用于「是否同一目录」的比较。

### 7.3 常量

`src-tauri/src/tool_data.rs`：

```rust
pub const TOOL_DATA_DIR_NAME: &str = "tool-data";
pub const TOOL_DATA_DB_NAME: &str = "tool-data.db";
pub const MAX_KV_VALUE_BYTES: usize = 256 * 1024;
pub const MAX_KV_KEYS_PER_TOOL: usize = 200;
```

前端不需要这些常量：位置信息由 `get_tool_data_location` 返回。

---

## 8. 运行时状态与并发

### 8.1 状态结构

`src-tauri/src/tool_data.rs`：

```rust
pub struct ToolDataState {
    inner: RwLock<StoreInner>,
    switching: AtomicBool,
}

struct StoreInner {
    root: PathBuf,
    conn: Option<Connection>,
    unavailable_reason: Option<String>,
}

#[derive(Clone)]
pub struct ToolDataStore(pub Arc<ToolDataState>);
```

`conn` 为 `Option`：`None` 表示已打开失败或正在切换，读命令据此返回错误。

### 8.2 访问协议

```rust
impl ToolDataStore {
    // 读路径：不阻塞，切换中立即失败
    pub fn with_conn<T>(&self, f: impl FnOnce(&Connection) -> Result<T, String>) -> Result<T, String> {
        if self.0.switching.load(Ordering::Acquire) {
            return Err(assistant_i18n::t("error.tool_data_busy"));
        }
        let guard = self.0.inner.read().map_err(|_| assistant_i18n::t("error.tool_data_busy"))?;
        let conn = guard.conn.as_ref().ok_or_else(|| {
            assistant_i18n::t("error.tool_data_unavailable")
        })?;
        f(conn)
    }
}
```

`switching` 置位先于获取写锁，确保切换期间的读命令**立刻失败**而不是在 `RwLock` 上排队（不变式 3）。切换耗时与数据量成正比，排队会让前端卡住数秒。

### 8.3 打开与初始化

```rust
pub fn open_at(root: &Path) -> Result<Connection, String>
```

1. `create_dir_all(root)`。
2. `Connection::open(root.join(TOOL_DATA_DB_NAME))`。
3. `PRAGMA key = "x'<hex(HKDF(TOOL_DATA_KEY_SEED))>'"`（见 §6.5）。
4. `PRAGMA journal_mode = DELETE`（显式设置，不依赖默认值）。
5. `PRAGMA synchronous = FULL`。
6. 试探解密：`SELECT count(*) FROM sqlite_master`，失败即 `error.tool_data_open_failed`。
7. 建表（§6.2）。
8. 读/写 `tool_data_meta.schema_version`。

`synchronous = FULL` 的理由：便签是用户手写内容，断电丢最后一条不可接受；工具数据量小，`FULL` 的额外开销可忽略。

`journal_mode` 保持 `DELETE` 而非 `WAL`：`DELETE` 模式下关闭连接即保证文件静止，迁移只需处理单文件；`WAL` 需先 checkpoint 才能拷贝，多一个失败点。

`PRAGMA key` 之后 SQLCipher 不做立即校验，首次读写才真正解密，因此步骤 6 的 `sqlite_master` 查询就是那次试探。由于密钥恒定，此步失败只可能是**库损坏或非本应用生成的文件**，与「密码错误」无关——错误键因此是 `open_failed` 而非 `key_unavailable`。

### 8.4 启动

`setup.rs` 在 `AppConf::load()` 之后、`manage(DbState)` 附近：

```rust
let store = tool_data::init(&handle);
handle.manage(store);
```

`init` 内部：解析根目录 → `open_at` → 成功则 `conn = Some`，失败则 `conn = None` 且记录 `unavailable_reason`，**不阻断启动**（不变式 5）。失败原因写 `log::warn` 并在设置页展示。

---

## 9. 目录重定位

### 9.1 命令

```rust
#[command]
pub fn get_tool_data_location(app: AppHandle) -> Result<ToolDataLocationDto, String>

#[command]
pub fn set_tool_data_dir(
    app: AppHandle,
    dir: Option<String>,       // None = 恢复默认
    keep_old_copy: Option<bool>, // 默认 false
) -> Result<ToolDataLocationDto, String>
```

`ToolDataLocationDto`（放入 `src-tauri/src/protocol/dto/storage.rs`，沿用该文件的 serde 约定）：

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolDataLocationDto {
    pub dir: String,             // 解析后的绝对路径
    pub custom: bool,            // conf.tool_data_dir.is_some()
    pub state: String,           // ready | unavailable
    pub db_bytes: u64,           // tool-data.db 文件大小，设置页展示
    pub error: Option<String>,   // state = unavailable 时的原因
}
```

前端 `toolDataService` 用 `dir` / `custom` / `state` / `dbBytes` / `error` 读取。**不含任何加密/保护字段**：本单元只有混淆，不暴露档位（§6.5）。

**为什么不用 `patch_conf`**：`patch_conf` 是「先落盘配置、再跑侧效」，侧效失败不回滚（§4）。目录切换可能因目标不可写、拷贝失败而失败，若走该路径就会留下配置与运行状态不一致。本命令自己编排顺序，配置只在切换成功后写入。

### 9.2 校验（`validate_target`）

按顺序执行，任一失败即返回对应错误码：

| # | 规则 | 错误键 |
|---|---|---|
| 1 | 非空、绝对路径、不含 `..` | `error.tool_data_invalid_path` |
| 2 | 若已存在，必须是目录而非文件 | `error.tool_data_not_a_directory` |
| 3 | 若不存在，父目录必须存在且可写 | `error.tool_data_parent_not_writable` |
| 4 | 目标目录可写：写 `.write-test` 后删除 | `error.tool_data_not_writable` |
| 5 | 与当前根目录相同（canonical 比较，失败则字符串比较）→ 视为重试，直接重开连接并返回 | — |
| 6 | 目标目录已含 `tool-data.db` 且不是当前根 → 拒绝，避免覆盖未知数据 | `error.tool_data_target_occupied` |
| 7 | 目标与当前根存在包含关系（一个在另一个内部）→ 拒绝，避免拷贝递归 | `error.tool_data_nested_path` |

**网络盘不拦截**（不变式 12）：`\\` 开头、`DRIVE_REMOTE` 盘符、同步盘目录**一律允许选择**，只在前端给一次提示（见 §9.2.1）。理由：路径形态无法可靠穷举（映射盘可能指向本地 NAS、`\\` 也可能指本机共享），硬拦截会误伤；且用户对自己选的目录负责。

规则 7 的理由：把新目录设在旧目录内部（或反之）会让拷贝过程自我包含，必然损坏数据。

### 9.2.1 风险路径提示（不拦截）

`ToolDataLocationDto` 增加一个**只读提示字段**，不参与校验：

```rust
// 追加到 ToolDataLocationDto（§9.1）
pub warning: Option<String>,   // 风险提示键，None = 无提示
```

| 判定 | `warning` |
|---|---|
| UNC 路径（`\\` 开头） | `settings.storage.tool_data.warn_network` |
| Windows `DRIVE_REMOTE` 盘符 | `settings.storage.tool_data.warn_network` |
| 其它（本地盘、未知） | `None` |

前端在确认弹窗里以普通提示色展示（非错误色），文案说明「该位置可能是网络盘，数据库在此类介质上可能变慢或出现锁冲突」。用户确认后照常执行，**不阻断、不要求二次勾选**。

判定只用于提示，因此**允许失败**：平台 API 不可用或返回未知类型时，按「无提示」处理，绝不因判定失败而拒绝路径。

### 9.3 迁移流程

```
1. switching.store(true)
2. 获取写锁
3. 记录旧状态：old_root、old_conn、old_counts（note_entries / tool_kv 行数）
4. 关闭旧连接（drop old_conn）        // DELETE 模式下关闭即可保证文件静止
5. 若 old_conn 为 None（原为 unavailable）→ 跳过 6~9，直接到 10
6. copy(old_root/tool-data.db → target/tool-data.db)   // 规则 6 已排除目标已有库
7. open_at(target) → 得到 new_conn
8. 校验：PRAGMA integrity_check == "ok"
        且 note_entries / tool_kv 行数与 old_counts 相等
9. 校验失败 → 清理本次新建的 target/tool-data.db → 重开 old_root → 返回错误
10. 更新 inner{ root, conn } → AppConf.tool_data_dir 写入并 save()
11. 若 !keep_old_copy 且 old_root 位于默认位置（app_data_dir/tool-data）→ remove_dir_all(old_root)
    若 old_root 是用户指定的外部目录 → 不删除（不变式 8）
12. 释放写锁后清除 switching
13. emit `tool-data:location-changed`，载荷 ToolDataLocationDto
```

要点：

- **先关后拷**：`DELETE` 模式下没有 `-wal` 待落盘内容，关闭连接即代表文件静止，拷贝无需额外快照。
- **单文件即可搬完**：混淆密钥在二进制内，不随目录走，因此迁移只处理 `tool-data.db` 一个文件，不存在「成对拷贝」的失败模式（不变式 11）。
- **拷贝后校验行数**，不校验字节：字节受 SQLite 内部页布局影响，行数等价性才是「数据都在」的可判定条件。
- **回滚顺序**：先恢复旧连接再清理目标副本；反过来会先删掉唯一成功副本。顺序错误即为实现错误。
- **外部旧目录不删**：用户选的目录可能还有别的东西，应用无权删除。返回的 DTO 带 `dir` 新路径，旧路径由前端在成功提示里提示「原位置数据保留在 X，可自行删除」。

### 9.4 恢复默认

`set_tool_data_dir(None, ...)`：

- 默认根 = `app_data_dir/tool-data`。
- 若默认根不存在：直接创建并切换，无拷贝。
- 若默认根存在且含 `tool-data.db`：按 §9.2 规则 6 会被拒绝——此处是**例外**：默认根是应用自管位置，允许覆盖。改为「以当前活动数据为准拷贝到默认根，覆盖前先把已存在的 `tool-data.db` 重命名为 `tool-data.db.prev`」。这条例外只对默认根生效，用户目录仍遵守规则 6。

### 9.5 前端

`src/services/toolDataService.ts`：

```ts
export const toolDataService = {
  getLocation: (): Promise<ToolDataLocationDto> => invoke("get_tool_data_location"),
  setDir: (dir: string | null, keepOldCopy = false): Promise<ToolDataLocationDto> =>
    invoke("set_tool_data_dir", { dir, keepOldCopy }),
  get: <T>(toolId: string, key: string): Promise<T | null> =>
    invoke("tool_kv_get", { toolId, key }),
  set: (toolId: string, key: string, value: unknown): Promise<void> =>
    invoke("tool_kv_set", { toolId, key, value }),
  remove: (toolId: string, key: string): Promise<void> =>
    invoke("tool_kv_delete", { toolId, key }),
  keys: (toolId: string): Promise<string[]> =>
    invoke("tool_kv_list", { toolId }),
};
```

设置页 `StorageSettings` 顶部新增「工具数据位置」分组：

| 元素 | 行为 |
|---|---|
| 当前位置 | 只读文本 + 复制按钮；`state === "unavailable"` 时显示红色原因 |
| 来源标记 | `custom` 为真显示「自定义」，否则「默认」 |
| 占用 | `db_bytes` 经 `formatBytes` |
| 更改位置 | `open({ directory: true, multiple: false })` → 二次确认弹窗（含迁移说明、「保留原位置副本」勾选、`warning` 提示）→ `setDir` |
| 恢复默认位置 | 确认弹窗 → `setDir(null)` |
| 切换中 | 按钮进入 loading，禁用重复点击 |

**不展示任何加密/保护状态**：本单元只有混淆（§6.5），设置页若写「已加密」即为误导。

切换前若当前路由是便签工具页，需先 flush 未保存内容；但设置页与工具页是互斥路由，实际不会同时存在，故不加跨页 flush 逻辑。

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
| `key` 长度 > 128 | 拒绝，`error.tool_data_key_too_long` |
| 序列化后字节 > `MAX_KV_VALUE_BYTES` | 拒绝，`error.tool_data_value_too_large` |
| 该 `tool_id` 键数已达 `MAX_KV_KEYS_PER_TOOL` 且本次为新增 | 拒绝，`error.tool_data_too_many_keys` |
| 写入 | `INSERT ... ON CONFLICT(tool_id, key) DO UPDATE`，同时更新 `updated_at` |

`tool_id` 白名单字符的理由：它是命名空间主键，也是未来 `files/<tool_id>/` 的目录名，必须能安全地当路径分量。

### 10.3 工具侧封装

工具目录内直接调 `toolDataService`，不额外加 hook：

```ts
// src/tools/clock/clock.service.ts 中的示例用法
await toolDataService.set("clock", "lastDurationMs", 25 * 60_000);
```

`clock` 的运行态按 I26 §9.2 仍在 Rust 内存，KV 只用于「上次选的时长」这类无关键状态，属可选。

---

## 11. 与便签（I26）的边界

I26 的 `note_entries` 建在 `tool-data.db`，不在 `assistant.db`。分工如下：

| 层 | 内容 | 位置 |
|---|---|---|
| 本规格 | 存储单元、目录解析、连接与切换、KV | `tool-data.db` 与 `ToolDataStore` |
| I26 | `note_entries` 表结构、便签命令、前端界面 | 建表语句并入本规格 §6.2 |

- 便签是用户产出内容，最可能需要放到别的位置；留在主库就无法单独搬。
- 便签表被删除或损坏时不应牵连会话数据。
- 便签**不做加密**：混淆只让通用工具打不开，不构成保护。需要保密的内容应进 vault（§3）。

对齐点：I26 §10.2 的建表语句加入 §6.2 的建库批次；I26 §10.3 的便签命令经 `ToolDataStore::with_conn` 取连接，不用 `DbState`。

因两份规格均未实现，无需写数据迁移脚本。

---

## 12. 扩展点（v1 不实现）

以下留接口不改协议：

| 扩展 | 预留 |
|---|---|
| 二进制文件 | `<root>/files/<tool_id>/` 目录约定已在 §6.1 声明；新增 `tool_file_*` 命令即可，不动既有命令 |
| 工具专用表 | 在 `tool_data::schema` 中追加 `CREATE TABLE IF NOT EXISTS`，并沿用 ADD COLUMN 迁移纪律 |
| 按工具统计占用 | `tool_data_meta` 已可承载；`get_storage_breakdown` 增加 `toolDataBytes` 字段（见 §13） |

---

## 13. 其它改动

| 项 | 改动 |
|---|---|
| `get_storage_breakdown` | `StorageBreakdownDto` 增加 `toolDataBytes: number`，值为 `tool-data.db` 文件大小；前端在「工具数据位置」分组展示 |
| i18n | `settings.storage.tool_data.*`（标题、位置、默认、自定义、更改、恢复默认、迁移确认、成功、原位置保留提示、`warn_network`）写入 `src/i18n/locales/{zh-CN,en-US}.json`；`error.tool_data_*`（`busy` / `unavailable` / `open_failed` / `invalid_path` / `not_a_directory` / `parent_not_writable` / `not_writable` / `target_occupied` / `nested_path` / `invalid_tool_id` / `key_too_long` / `value_too_large` / `too_many_keys`）写入 `src-tauri/crates/assistant-i18n/locales/{zh-CN,en-US}.json`，两套 locale 各自键集一致 |

---

## 14. 文件清单

新增：

| 文件 | 内容 |
|---|---|
| `src-tauri/crates/assistant-storage/src/tool_data/mod.rs` | 建库、KV 与 meta 读写 |
| `src-tauri/crates/assistant-storage/src/tool_data/kv.rs` | `tool_kv` 的 CRUD 与限额校验 |
| `src-tauri/crates/assistant-storage/src/tool_data/key.rs` | 混淆密钥派生（HKDF，常量 seed） |
| `src-tauri/src/tool_data.rs` | 根目录解析、打开、切换、迁移、`ToolDataStore` 状态 |
| `src-tauri/src/cmd/tool_data.rs` | 6 个命令（位置 2 + KV 4） |
| `src/services/toolDataService.ts` | 前端命令代理 |

修改：

| 文件 | 改动 |
|---|---|
| `src-tauri/crates/assistant-config/src/app_conf.rs` | `tool_data_dir` 字段 + `validate` |
| `src/settings/core/types/confTypes.ts` | `toolDataDir` |
| `src-tauri/src/setup.rs` | `tool_data::init` + `manage(ToolDataStore)` |
| `src-tauri/src/cmd/mod.rs` | 导出 `tool_data` 模块 |
| `src-tauri/src/ipc.rs` | 注册 6 个新命令 |
| `src-tauri/src/protocol/events.rs` | `TOOL_DATA_LOCATION_CHANGED` |
| `src-tauri/src/protocol/dto/storage.rs` | `ToolDataLocationDto` + `StorageBreakdownDto.tool_data_bytes` |
| `src-tauri/src/cmd/storage.rs` | `toolDataBytes` 计算 |
| `src/services/storageService.ts` | `StorageBreakdownDto.toolDataBytes` |
| `src-tauri/crates/assistant-storage/src/db.rs` | 无改动（便签表建在工具数据库，不在此处） |
| `src/settings/pages/StorageSettings.tsx` | 工具数据位置分组 |
| `src/types/events.ts` | `UiEvent.ToolDataLocationChanged` |
| `src/i18n/locales/{zh-CN,en-US}.json` | 键集 |

`Cargo.toml` 无新增依赖：`rusqlite`（已带 `bundled-sqlcipher`）、`hkdf`、`sha2`、`serde_json`、`thiserror`、`tokio` 均已在用。**不引入 `keyring`**（§6.5）。

---

## 15. 验收

静态检查：

- [ ] `npm run typecheck` / `npm run lint` / `npm run test` 通过
- [ ] `cargo fmt` 后 `cargo test`（cwd = `src-tauri`）通过

单元测试（Rust，用 `tempfile`）：

- [ ] `validate_target`：相对路径、含 `..`、指向文件、父目录不可写、目标已有库、目标在当前根内部 —— 六类各返回对应错误
- [ ] `validate_target` 对 UNC 路径与 `DRIVE_REMOTE` 盘符**不返回错误**，仅 `warning` 有值
- [ ] 迁移：源含 N 条便签与 M 个 KV → 迁到新目录后行数一致；源目录（默认位置）被删除；外部源目录不被删除
- [ ] 迁移失败注入（目标不可写）：活动连接仍指向原目录，原数据可读，目标无残留 `tool-data.db`
- [ ] `unavailable`：配置指向不存在的盘 → `init` 成功但 `conn = None`；KV 命令返回 `error.tool_data_unavailable`
- [ ] `set_tool_data_dir(Some(同一路径))` 在 `unavailable` 下能重开连接（即「重试」语义）
- [ ] 混淆：同一路径的库用通用 SQLite 打开失败；用 `PRAGMA key` 派生值打开成功
- [ ] KV 限额：超 256 KiB、超 200 键、非法 `tool_id` 均拒绝
- [ ] 切换期间并发：切换进行中调用 `tool_kv_set` 立即返回 `error.tool_data_busy`，不阻塞

手工回归：

- [ ] 默认位置下创建便签 → 设置页改为另一本地盘 → 重启 → 便签仍在，路径显示新位置
- [ ] 用 DB Browser 打开 `tool-data.db`：提示「file is not a database」，读不到任何内容
- [ ] 目录被删除后启动：应用正常进入，聊天可用，便签页显示不可用提示，设置页显示原因
- [ ] 恢复目录后点「更改位置」并选择同一路径 → 存储恢复 `ready`
- [ ] 选择 UNC 路径：出现网络盘提示，确认后仍可切换成功（不被拦截）
- [ ] 恢复默认位置后，原外部目录数据保留，提示文案指出旧路径
- [ ] 切换过程中重复点击按钮被禁用，无重复迁移

---

## 16. 未验证前提

1. **SQLite 在网络盘（SMB/NFS）上的可靠性**：路径已改为「只提示不拦截」，`DELETE` + `synchronous = FULL` 能降低风险但不能消除锁语义差异。需实测在网络盘上编辑便签是否出现 `database is locked`，以决定提示文案的强度。
2. **同步盘（OneDrive/Dropbox）的并发写冲突**：两个设备同时写同一 `tool-data.db` 会产生冲突副本。v1 只记录该风险，不做检测；需实测同步客户端是否会在文件被占用时反复重试。
3. **`remove_dir_all` 对默认位置旧目录的失败场景**：文件被外部程序占用时删除失败。需确认失败后仅记录日志（不回滚已完成的切换）是否可接受。
4. **`open({ directory: true })` 在 Windows 上返回的路径形式**：可能是 `\\?\` 前缀或正斜杠。需实测后统一规范化，否则「同一路径」判断会失效。
5. **非 UTF-8 路径**：`tool_data_dir` 存 `String`，极端路径无法表示。需确认是否有必要改为平台原生路径类型。
6. **迁移期间前端其它工具页并发读**：切换时 `switching` 立即失败，需确认失败提示对用户足够清晰，而不是被当成「工具坏了」。
7. **HKDF 派生值在 SQLCipher 的 `PRAGMA key` 中的编码**：以 `x'<hex>'` 传入 32 字节原始 key 是标准写法，需确认 `bundled-sqlcipher` 构建下行为一致（vault 已有同类用法可对照）。
8. **`DRIVE_REMOTE` 判定的覆盖范围**：映射盘、`subst` 盘、WebDAV 挂载的返回值不一致，判定只用于提示，需实测确认不会误报本机共享。
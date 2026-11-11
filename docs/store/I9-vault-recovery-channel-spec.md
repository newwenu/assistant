# I9 密码箱 — 恢复通道规格（原 key-hierarchy-spec §3.8 + §6 迁出）

> 状态：规格（2026-11-11 分派）
> 来源：[I9-vault-key-hierarchy-spec.md](./I9-vault-key-hierarchy-spec.md) §2.5、§3.8、§6、§9.1 恢复命令、§9.2 恢复 DTO、§10 恢复 UI、§11.3。
> 上游：[I9-vault-key-hierarchy-design.md](./I9-vault-key-hierarchy-design.md)
> 同层：[I9-vault-plaintext-egress-spec.md](./I9-vault-plaintext-egress-spec.md)（恢复码展示与导出）、[I9-vault-audit-spec.md](./I9-vault-audit-spec.md)（恢复审计）
> 本规格只负责**纸面编解码、`RecoverySession`、恢复流程、恢复命令与 UI**。密钥层次的 `dek` 本身见 key-hierarchy-spec。

---

## 0. 范围与删除清单

### 0.1 覆盖

| 落点 | 文件 |
|---|---|
| 纸面编解码 | `src-tauri/crates/assistant-vault/src/recovery_key/mod.rs` |
| `RecoverySession` | 同上 |
| 恢复命令 | `src-tauri/src/cmd/vault/recovery.rs` |
| DTO | `src-tauri/crates/assistant-vault/src/dto/recovery.rs`（已存在 `DisasterRecoveryResultDto`；扩展新 DTO，并删除随 `vault_disaster_recovery_export` 失效的旧 DTO） |
| 前端 | `src/vault/unlock/DisasterRecovery.tsx`、`src/vault/settings/RecoveryTab.tsx`、`src/vault/core/vault.service.ts` |

### 0.2 删除

`recovery_key/mod.rs` 删除：

- `generate_recovery_key`（`:33`）
- `decrypt_recovery_keys`（`:79`）
- `unlock_with_recovery_key`（`:127`）
- `EmergencyData`（`:27-31`）
- `derive_recovery_key` / `parse_recovery_key` / `format_recovery_key`（v1 32 字符 4 组编码）
- `recovery_key/mod.rs:191` 的 `PRAGMA` 字面量（统一走 `SQLCIPHER_PRAGMAS`）
- `lib.rs` 中 `unlock_with_recovery_key` 的再导出（若有）

命令层删除：

- `vault_generate_recovery_key`
- `vault_has_recovery_key`
- `vault_verify_vault_meta_integrity`
- `vault_disaster_recovery_export`

同步层删除（与 sync-crypto-spec 对齐）：

- `SyncEngine::write_emergency_data` / `read_emergency_data` / `clear_emergency_data`
- `SyncEngine::disaster_recovery_export` 改为接受 `RecoverySession`

---

## 1. 纸面编解码

```rust
pub const PAPER_CHARS: usize = 40;
pub const PAPER_GROUPS: usize = 5;
pub const PAPER_GROUP_LEN: usize = 8;
pub const PAPER_VERSION_BYTE: u8 = 0x02;
pub const CROCKFORD_ALPHABET: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

pub fn encode_paper(dek: &Dek) -> String;                       // 40 字符，无分隔
pub fn format_groups(key: &str) -> Vec<String>;                 // 5 × 8
pub fn normalize_paper_input(raw: &str) -> String;
pub fn decode_paper(raw: &str) -> Result<Dek, VaultError>;
pub fn locate_checksum_error(raw: &str) -> Option<(u8, u8)>;    // (组, 位)，均 1-based
```

### 1.1 字节布局

25 字节 → 200 bit → 40 字符（恰好整除）：

```
version(1) = 0x02 ‖ dek(20) ‖ crc32c(4, 大端)
```

分组示例（`-` 仅用于展示，解析时忽略）：

```
K7M2QXF9-3TBAWZE5-RNCJ4YD6-PHK2SVG9-MTB7XQFD
└──── 8 ────┘ × 5 组
```

### 1.2 CRC32C 参数

`crc::CRC_32_ISCSI`（Castagnoli）：

| 项 | 值 |
|---|---|
| 多项式（反射） | `0x82F63B78` |
| init | `0xFFFFFFFF` |
| xorout | `0xFFFFFFFF` |
| 反射输入/输出 | 是 |
| 计算范围 | 前 **21** 字节（`version ‖ dek`） |

依赖：`assistant-vault/Cargo.toml` 新增 `crc = "3"`，workspace `Cargo.toml` 同步新增。若不愿增依赖，可手写 256 项查表实现（参数同上），但**禁止改用非 CRC32C 的多项式**（否则与设计 §4.4 的「2^-32 漏检」论证不符）。

### 1.3 归一化规则（顺序不可变）

1. 全角 `０-９`、`Ａ-Ｚ` → 半角（`U+FF10..U+FF19` → `0..9`，`U+FF21..U+FF3A` → `A..Z`）
2. 转大写
3. 删除 `-`、空格、`\t`、`\r`、`\n`、全角空格 `U+3000`
4. `O` → `0`
5. `I` / `L` → `1`
6. `U` → `V`

必须实现；否则用户抄写必失败。

### 1.4 解码校验顺序（顺序决定错误码，不可调换）

| 步骤 | 失败错误码 |
|---|---|
| ① 归一化后长度 ≠ 40 | `InvalidRecoveryKeyFormat` |
| ② 含字母表外字符 | `InvalidRecoveryKeyFormat` |
| ③ base32 解码后长度 ≠ 25 | `InvalidRecoveryKeyFormat` |
| ④ `bytes[0] != 0x02` | `InvalidRecoveryKeyFormat` |
| ⑤ CRC32C 不符 | `InvalidRecoveryKeyChecksum` |
| ⑥ 通过 | 返回 `Dek` |

### 1.5 `locate_checksum_error`（纠错定位）

1. 对归一化后的每个位置 `i ∈ 0..40`，逐个尝试字母表中除原字符外的 **31** 个替换值；
2. 对每个候选执行 ②–⑤；
3. **恰好一个**候选通过 CRC → 返回 `(i / 8 + 1, i % 8 + 1)`；
4. 零个或多个候选通过 → 返回 `None`（UI 只提示「校验失败」，不猜位置）。

约束：

1. `locate_checksum_error` **不得触达任何口令相关代码路径**。它只做 base32 + CRC32C。
2. 复杂度上界 40 × 31 = 1240 次 CRC，单次 < 1µs，无需优化。
3. `decode_paper` 返回 `Dek` 时**不校验 HMAC**——HMAC 校验在 §3 的恢复流程里，因为需要读 `.vault-meta`。

### 1.6 为什么是 160 bit / 为什么不用助记词

- 40 字符 = 5 组，正好「一行写完、一次抬头扫读」；60 字符（256 bit + 校验）会明显增加抄错率。
- 160 bit 对**随机生成**的密钥无条件安全；不涉及口令字典，无需 Argon2id 加固。
- Base32 + CRC 在「短、可定位、无语言依赖」三项上都优于 BIP39；恢复密钥的使用场景是**从纸上抄进输入框**，不需要记忆。
- 代价：`entryKey = HKDF(dek, …)` 的有效熵上限为 160 bit；对 AES-256 不是弱点，但文档必须说清，不要假装是 256 bit。

### 1.7 「可验证性」与预言机

**本设计只放 CRC32C（防抄错），不在任何存储介质上放 `dek` 的哈希。**

- CRC32C 线性、可伪造：给定任意 `dek` 都能算出配套 CRC，因此它**不能**证明密钥正确；但「抄错必然被发现」（漏检概率 2^-32）。
- `SHA256(dek)` 一旦落到仓库，就等价于公开一个 2^160 的哈希靶，且每次猜测只需一次 SHA256——净损失。
- 一句话：**恢复密钥不需要「可验证」，它需要「可抄对」。**

CRC 的真实代价是「`dek` 候选的廉价验证通道」：已持有绝大部分 `dek`（如照片糊了 3 位）时可穷举 31³ ≈ 3 万次还原。但它**只放大已泄露的纸面，不触及口令**；且 CRC 只在纸面，不在仓库。

---

## 2. `RecoverySession`

位置：`src-tauri/crates/assistant-vault/src/recovery_key/mod.rs`。

```rust
pub struct RecoverySession {
    dek: Dek,                       // 唯一密钥材料
    entries_dir: PathBuf,           // 只读：entries/*.enc 所在目录
    manifest: VaultManifest,        // 只读：已校验过的 manifest
}

impl RecoverySession {
    pub fn export_all(&self) -> Result<Vec<PlainEntry>, VaultError>;
    pub fn list_ids(&self) -> Result<Vec<String>, VaultError>;
}
```

约束：

1. **不持有 `SecurityDbState`、不持有 `Connection`、不持有 `DbKey`**。设计只写 `dek` 一个字段；仅持 `dek` 时 `export_all` 无法读磁盘，故补 `entries_dir` 与 `manifest`（均为只读、无密钥）。
2. **禁止提供** `conn()` / `db()` / `exec()` / `audit()` / `db_key()`。`db_key` 拿不到是编译期事实，不是运行时约定。
3. `PlainEntry` 定义在 `assistant-vault/src/types.rs`：

```rust
pub struct PlainEntry {
    pub id: String,
    pub category: String,
    pub blob: EntryBlob,        // 复用 vault_file::types::EntryBlob
}
```

> 权限口径：v1.5 的 `dbKey = HKDF(KEK)` 使纸面在密码学上无法开库；若后续改为 `dbKey = HKDF(dek)`，则 `RecoverySession` 的隔离退化为**API 层最小权限**，文档必须同步改口径，不得继续宣称「密码学上不可能开库」。

---

## 3. 恢复流程

### 3.1 `open_recovery_session`

```rust
pub fn open_recovery_session(
    repo: &VaultFileManager,
    raw_key: &str,
) -> Result<RecoverySession, VaultError>;
```

```
1. dek = decode_paper(raw_key)
   ├─ 格式错 ──▶ InvalidRecoveryKeyFormat
   └─ CRC 错 ──▶ InvalidRecoveryKeyChecksum
2. read_vault_meta_v2
   ├─ version=1 ──▶ UnsupportedVaultVersion
   └─ 其余非法 ──▶ VaultCorrupted
3. metaKey = derive_meta_key(&dek)
   verify_meta_integrity(metaKey)
   └─ 失败 ──▶ InvalidRecoveryKeyOrMetaTampered
4. verify_manifest()（逐条 sha256）
   └─ 失败 ──▶ VaultManifestMismatch
5. 构造 RecoverySession { dek, entries_dir, manifest }
```

约束：

1. **第 3 步的错误不可拆分**：解码后的 `dek` 恒为 20 字节，长度不构成区分依据；CRC 已过而 HMAC 失败只可能是「纸面抄错但 CRC 侥幸碰撞」或「meta 被篡改」，统一返回一个码，UI 同时提示两种可能。
2. **完全不触碰 `security.db`**：不派生 `dbKey`、不调 `PRAGMA`、不读 anchor。
3. `open_recovery_session` **不写审计库**（没有连接）；导出结束后写 JSONL。

### 3.2 `RecoverySession::export_all`

```
1. 遍历 manifest.entries（按 id）
2. entryKey = derive_entry_key(&dek, id)
3. 读 entries/{id}.enc → decrypt_entry_blob → EntryBlob
4. 校验文件 sha256 与 manifest 一致（防止 3 与 1 之间被替换）
   └─ 不符 ──▶ VaultManifestMismatch
5. 返回 Vec<PlainEntry>
```

约束：

1. **不做任何写入**。连 `recoveryBackup` 标记都不写（那是导出纸面的动作，不是用纸面的动作）。
2. 单个条目解密失败 → 记入 `skipped`，**不中断整体**（与 v1 `export_all_entries` 的 `skipped_ids` 语义一致）。
3. 结束时调用方写 JSONL `RecoveryExportUsed { entryCount, at }`。

### 3.3 `recoveryBackup` 标记

| 动作 | 写入者 | 内容 |
|---|---|---|
| `vault_export_recovery_key` 成功返回纸面 | 命令层 → `write_vault_meta_v2` | `recoveryBackup = { dek_version: 当前, confirmedAt: now }` |
| `rotate_dek` | `lifecycle::rotate_dek` | `recoveryBackup = None` |
| `initialize` | `lifecycle::initialize` | `recoveryBackup = None` |

「是否已备份」的查询：当前代码**没有** `VaultStatusDto` / `vault_get_status`（全景清单 F22），因此新增一个最小命令：

```rust
fn vault_has_recovery_backup(app: AppHandle) -> Result<Option<RecoveryBackupDto>, VaultErrorDto>;
```

- 返回 `None` = 从未导出过纸面，或当前 `dekVersion` 已变化（视为未备份）；
- 返回 `Some` = 已确认备份当前 `dekVersion`；
- 只读 `.vault-meta`，不需要解锁；
- 不新增通用 `vault_get_status`，避免扩大契约面。

---

## 4. 命令层

### 4.1 命令增删

`src-tauri/src/cmd/vault/recovery.rs`：

| 命令 | 处理 | 签名 |
|---|---|---|
| `vault_export_recovery_key` | **新增** | `fn vault_export_recovery_key(app, master_password: String) -> Result<RecoveryKeyDto, VaultErrorDto>` |
| `vault_recover_from_paper` | **新增** | `async fn vault_recover_from_paper(app, paper_key: String) -> Result<RecoveryExportDto, VaultErrorDto>` |
| `vault_locate_paper_error` | **新增** | `fn vault_locate_paper_error(paper_key: String) -> Result<Option<PaperErrorPosDto>, VaultErrorDto>` |
| `vault_has_recovery_backup` | **新增** | `fn vault_has_recovery_backup(app: AppHandle) -> Result<Option<RecoveryBackupDto>, VaultErrorDto>` |
| `vault_generate_recovery_key` | **删除** | — |
| `vault_has_recovery_key` | **删除** | — |
| `vault_verify_vault_meta_integrity` | **删除** | — |
| `vault_disaster_recovery_export` | **删除** | — |

> `vault_rotate_dek` 由 key-hierarchy-spec 的编排层负责；它返回的纸面使用本规格的 `RecoveryKeyDto`。

命令注册点同步更新：`src-tauri/src/vault_plugin.rs` 的 `generate_handler!`。

### 4.2 DTO

`src-tauri/crates/assistant-vault/src/dto/recovery.rs`（**已存在**，`dto/mod.rs` 已声明 `pub mod recovery;`；在其上扩展并删除旧 `DisasterRecoveryResultDto`）：

```rust
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryKeyDto {
    pub paper_key: String,          // 40 字符，无分组
    pub groups: Vec<String>,        // 5 × 8，UI 直接渲染
    pub dek_version: u32,
    pub created_at: i64,            // 毫秒
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryExportDto {
    pub entries: Vec<RecoveryEntryDto>,
    pub skipped: Vec<RecoverySkipDto>,
    pub exported_at: i64,
    pub dek_version: u32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryEntryDto {
    pub id: String,
    pub category: String,
    pub blob: serde_json::Value,    // EntryBlob 的 JSON 形态
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoverySkipDto {
    pub id: String,
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaperErrorPosDto {
    pub group: u8,                  // 1-based
    pub index: u8,                  // 1-based
}
```

`vault_has_recovery_backup` 的返回类型：

```rust
#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub struct RecoveryBackupDto {
    pub dek_version: u32,
    pub confirmed_at: i64,     // 毫秒
}
```

同时更新 `dto/mod.rs` 的再导出：

```rust
pub use recovery::{DisasterRecoveryResultDto, RecoveryBackupDto, RecoveryExportDto, RecoveryKeyDto, PaperErrorPosDto};
```

---

## 5. 前端

### 5.1 `vault.service.ts`

| 方法 | 处理 |
|---|---|
| `generateRecoveryKey()` | **删除** → `exportRecoveryKey(masterPassword: string): Promise<RecoveryKeyInfo>` |
| `hasRecoveryKey()` | **删除** → `hasRecoveryBackup(): Promise<RecoveryBackupInfo \| null>`（调 `vault_has_recovery_backup`） |
| `disasterRecoveryExport(key)` | **删除** → `recoverFromPaper(paperKey: string): Promise<RecoveryExportResult>` |
| — | **新增** `locatePaperError(paperKey: string): Promise<PaperErrorPos \| null>` |

```ts
export interface RecoveryKeyInfo {
  paperKey: string;
  groups: string[];
  dekVersion: number;
  createdAt: number;
}

export interface RecoveryExportResult {
  entries: { id: string; category: string; blob: unknown }[];
  skipped: { id: string; reason: string }[];
  exportedAt: number;
  dekVersion: number;
}

export interface RecoveryBackupInfo { dekVersion: number; confirmedAt: number; }

export interface PaperErrorPos { group: number; index: number; }
```

### 5.2 组件

| 组件 | 职责 | 约束 |
|---|---|---|
| `RecoveryKeyDialog.tsx`（新建） | 展示 5 组纸面、复制、强制「我已抄写」勾选 | 关闭前未勾选 → 二次确认；**不提供「稍后再说」直接关闭** |
| `PaperRecoveryFlow.tsx`（新建） | 输入 → `locatePaperError` 定位 → 失败提示 → 导出 | 定位结果只在**校验失败**时展示；格式错不定位 |
| `DisasterRecovery.tsx` | 改为「纸面恢复」流程 | 输入 40 字符 → 只读预览 → 导出条目 |
| `RecoveryTab.tsx` | 「生成恢复密钥」改为「备份恢复密钥」；**移除完整性校验按钮**（`vault_verify_vault_meta_integrity` 已删除，props `integrityResult` / `verifyIntegrity` 一并移除） | 展示当前 `dek`；新增「轮换数据密钥」独立入口 + 强警告 |
| `RecoveryKeyForm.tsx`（新建） | 纸面输入 | 实时归一化、分组显示、CRC 错误提示到组位 |

### 5.3 i18n

```
vault.recovery.export.title / desc / copy / copied / confirmWritten
vault.recovery.paper.inputLabel / inputHint / locateSuccess / locateFailed / checksumFailed / formatFailed
vault.recovery.export.success / skippedCount
vault.rotate.title / desc / confirm / success / biometricReenrollRequired
vault.error.invalidRecoveryKeyFormat / invalidRecoveryKeyChecksum / invalidRecoveryKeyOrMetaTampered
```

约束：**禁止在组件内写中文字面量**；`vault.error.*` 与 camelCase 错误码一一对应。

---

## 6. 错误码

| 错误码 | 触发 | retryable |
|---|---|---|
| `InvalidRecoveryKeyFormat` | 长度/字符/版本字节/base32 长度错 | 否 |
| `InvalidRecoveryKeyChecksum` | CRC32C 不符 | 否 |
| `InvalidRecoveryKeyOrMetaTampered` | 纸面抄错（CRC 侥幸通过）或 meta 篡改 | 否 |

删除：`InvalidRecoveryKey`（无调用点后删除）、`RecoveryKeyNotSet`（唯一调用点随 `vault_disaster_recovery_export` 删除）。

前端数组 `src/vault/core/vaultError.ts` 同步：追加 3 项、删除 2 项。

---

## 7. 测试

| 编号 | 用例 | 断言 |
|---|---|---|
| T1 | `encode_paper` → `decode_paper` 往返（1000 次随机 `dek`） | 结果一致；长度恒 40 |
| T2 | 归一化：小写、`-`/空格、全角、`O/0`、`I/1/l/L`、`U/V` 混用 | 全部解出同一 `dek` |
| T3 | 单字符替换（40 位 × 31 值） | 全部 `InvalidRecoveryKeyChecksum` |
| T4 | 截断/超长/非法字符 | `InvalidRecoveryKeyFormat`，不 panic |
| T5 | CRC 正确的伪造 `dek` | 通过 CRC，但解不开任何条目 |
| T5b | 随机挖掉 1~3 个字符后定位 | 唯一还原；不触达口令代码路径 |
| I7 | `decode_paper(encode_paper(dek))` 往返 | 同一 `dek` |
| I8 | 6 级校验顺序 | 长度 39 → `Format`；40 字符含 `U` 归一化后 CRC 不符 → `Checksum` |
| I9 | 抄错一位（含 `O/I/L/U` 混淆） | `locate_checksum_error` 返回恰好一个位置，且等于实际错位 |
| I11 | `open_recovery_session` + `export_all` | 全部条目明文；**不产生任何 `security.db` 读写** |
| T-N1 | 全角/连字符/`O/I/L/U` | 全部正确映射 |
| I5 | 恢复会话 | 编译期拿不到 `dbKey`/连接/审计 |

测试落点：

- `src-tauri/crates/assistant-vault/tests/recovery_key.rs` **当前是 v1 32 字符/4 组测试**，本规格落地时整文件重写为 40 字符/5 组纸面测试（T1–T5b）；
- `open_recovery_session` / `export_all` 的集成用例放入新增的 `src-tauri/crates/assistant-vault/tests/lifecycle_integration.rs`（与 key-hierarchy-spec 共用）；
- 前端归一化/分组用例放入 `src/vault/core/__tests__/paperKey.test.ts`（新增）。

---

## 8. 禁止事项

1. **禁止**在仓库/`security.db` 中存放 `dek` 的哈希。
2. **禁止** `RecoverySession` 提供任何 DB 句柄访问器。
3. **禁止**让 `decode_paper` / `locate_checksum_error` 触达任何口令派生代码。
4. **禁止**把 CRC 改用非 CRC32C 多项式。
5. **禁止**在恢复流程中写入 `.vault-meta`（除导出纸面的 `recoveryBackup` 标记）。
6. **禁止**为 v1 恢复码保留兼容解析。
7. **禁止**在组件内硬编码中文文案。

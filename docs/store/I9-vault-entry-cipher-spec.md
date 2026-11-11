# I9 密码箱 — 条目层加密规格（S2）

> 状态：规格（2026-11-11）
> 上游：[I9-vault-key-hierarchy-design.md](./I9-vault-key-hierarchy-design.md)、[I9-vault-key-hierarchy-spec.md](./I9-vault-key-hierarchy-spec.md)
> 上下游：[I9-vault-sync-crypto-spec.md](./I9-vault-sync-crypto-spec.md)（仓库完整性）、[I9-vault-runtime-session-spec.md](./I9-vault-runtime-session-spec.md)（内存驻留）
> 本规格只定义**条目字节如何被加密、如何被认证、存在哪两个副本、两者如何保持一致**。密钥层次（`dek`/`KEK`）不在本文重复。
> 路径约定：全部为仓库相对路径。

---

## 0. 范围

覆盖 S2 面的全部落点：

| 落点 | 文件 | 说明 |
|---|---|---|
| 本机库加密列 | `security.db` 的 `vault_entries` | `encrypted_password` / `encrypted_notes` / `encrypted_totp_secret` / `encrypted_custom_fields` |
| 本机库明文字段 | 同上 | `title` / `username` / `url` / `tags` / `category` / `revision` / 时间戳 |
| 仓库条目文件 | `entries/{id}.enc` | `EntryFile` JSON，内含 `encryptedBlob` |
| 条目密钥 | `derive_entry_key` | 由 `dek`（v1.5）或 `master_key`（v1）派生 |
| 字段级 AAD | 新增 | 绑定条目 ID 与字段名 |
| 回收站/历史恢复 | `entries/deleted.rs`、`sync_engine/history.rs` | 复用同一套加密，不得出现旁路 |

明确不覆盖：`.vault-meta`（S3）、恢复纸面（S1）、导出文件（S5）、剪贴板/揭示（S4/S5）。

---

## 1. 现状与缺陷

### 1.1 现状

```
DB 行（SQLCipher 整库加密）
├─ title / username / url / tags          ← 明文字段（仅靠整库页加密）
├─ encrypted_password                      ← AES-GCM(algo‖nonce‖ct)  base64
├─ encrypted_notes                         ← 同上
├─ encrypted_totp_secret                   ← 同上，值为 TOTP payload 字符串
└─ encrypted_custom_fields                 ← JSON 数组；受保护项内嵌 base64 密文

entries/{id}.enc（普通 JSON）
{
  "id": "...",
  "category": "login",
  "revision": 3,
  "encryptedBlob": "base64(algo‖nonce‖ct)"
}
其中 ct = AES-GCM(entryKey, JSON(EntryBlob))
entryKey = HKDF(masterKey, "entry:" + id)      // v1；v1.5 改为 HKDF(dek, "i9:v1:entry:" + id)
```

### 1.2 缺陷

> 本表用 **E1–E7** 编号，避免与全景清单的 F1–F22 混淆。

| 编号 | 问题 | 后果 | 证据 |
|---|---|---|---|
| **E1** | `EntryFile` 的 `id` / `category` / `revision` 在密文外，且未进 AAD | 仓库可被整块重放旧条目；`revision` 可被改成极大值抑制后续更新；`category` 可被篡改导致错误分类 | `vault_file/types.rs:3-10`、`vault_file/mod.rs:139-166` |
| **E2** | `EntryBlob` 的 `totpDigits` / `totpPeriod` 是冗余字段：`export_entry` 硬编码 6/30，`ImportedEntry::into_import` 完全忽略它们 | **不丢数据**——真实参数在 `totpSecret` 的 JSON payload（`shared/totp.rs`）里；但字段语义误导，后续可能被误当成权威值 | `vault_file/types.rs:12-40`、`vault_file/mod.rs:95-96,390-417`、`shared/totp.rs:7-14,59-67` |
| **E3** | 解密失败被静默吞掉 | 受保护自定义字段解密失败会被直接丢弃；UTF-8 非法时返回空串；用户看到「字段消失」而非「数据损坏」 | `entries/field_crypto.rs:19-29`、`security_db/crypto_ops.rs:225-254` |
| **E4** | 明文字段无成文策略 | 后续开发者可能误以为 `title` 已被字段级加密，或在未评估时把它改成密文导致搜索失效 | `security_db/schema.rs:9-27` |
| **E5** | 字段密文无字段绑定 | 同一 `entryKey` 下，`encrypted_password` 的密文可被搬到 `encrypted_notes` 并正确解密 | `vault_crypto/mod.rs:80-158` |
| **E6** | 两条写入路径不一致 | `create_entry_tx` 会同步写 `entries/*.enc`，`import_entries` 只写库；导入后未立即提交仓库 | `entries/write.rs:78-138`、`import_export.rs:75-113` |
| **E7** | `EntryFile` 无版本/算法字段 | 未来换密文格式时无法区分新旧文件；`algo` 藏在 base64 首字节，仓库层不可见 | `vault_crypto/mod.rs:107-113` |

> E1、E3、E5 是当前代码就存在的问题，不依赖 v1.5 是否实施；E2 只是冗余字段清理，不构成数据丢失。

---

## 2. 目标数据布局

### 2.1 `EntryFileV2`

位置：`src-tauri/crates/assistant-vault/src/vault_file/types.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryFileV2 {
    pub version: u32,          // 恒 2
    pub id: String,            // 32/36 字符 UUID 文本，与文件名去掉 .enc 后一致
    pub category: String,      // 封闭集合字符串，见 VaultCategory
    pub revision: i64,         // >= 0
    pub dek_version: u32,      // v1.5：>= 1；v1 文件无此字段
    pub cipher: String,        // "aes256Gcm" | "chacha20Poly1305"
    pub encrypted_blob: String // b64(nonce(12) ‖ ct ‖ tag(16))
}
```

约束：

1. `id` 必须等于文件名去后缀；不等 → `VaultCorrupted`。
2. `version` 缺失或非 2 → `UnsupportedEntryFileVersion`（不迁移，见 §6）。
3. `cipher` 必须与 `encrypted_blob` 实际使用的算法一致；解码后首字节不再携带算法（算法只在 `cipher` 字段）。这是与 v1 的**不兼容点**。
4. `revision < 0` → `VaultCorrupted`。
5. **`category` / `revision` 保留在密文外**：同步合并需要在不持有密钥的场景下比较版本；它们由 AAD 认证（§2.4），篡改会导致解密失败。

### 2.2 `EntryBlobV2`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryBlobV2 {
    pub schema: u32,                 // 恒 1（blob 内部结构版本）
    pub title: String,
    #[serde(default)] pub username: Option<String>,
    #[serde(default)] pub url: Option<String>,
    #[serde(default)] pub password: Option<String>,
    #[serde(default)] pub notes: Option<String>,
    #[serde(default)] pub custom_fields: Vec<BlobCustomField>,
    #[serde(default)] pub totp: Option<TotpBlob>,   // 取代 totp_secret/digits/period
    #[serde(default)] pub tags: Vec<String>,
    #[serde(default)] pub is_favorite: bool,
    #[serde(default)] pub created_at: i64,
    #[serde(default)] pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TotpBlob {
    pub secret: String,
    #[serde(default = "default_totp_digits")]  pub digits: u32,
    #[serde(default = "default_totp_period")]  pub period: u32,
    #[serde(default = "default_totp_algorithm")] pub algorithm: String, // "sha1" | "sha256" | "sha512"
}
```

理由：

- `totp` 结构体把「secret + 参数」绑定在一起，**清理 E2 的冗余字段**（当前真实参数已藏在 `totpSecret` 的 JSON payload 里，外层 `totpDigits/Period` 是死字段）；`totp_algorithm` 不再有歧义。
- `schema` 让 blob 内部可以演进，不影响 `EntryFile` 的版本判定。
- `title` / `username` / `url` / `tags` 同时存在于 blob 与 DB 明文列：blob 是权威源，DB 列是可重建索引（§2.3）。

### 2.3 DB 列策略（成文）

`vault_entries` 分两类字段，必须在 `schema.rs` 注释与本规格同时写明：

| 类别 | 字段 | 保护方式 | 允许出现在检索/排序中 |
|---|---|---|---|
| **字段级密文** | `encrypted_password` / `encrypted_notes` / `encrypted_totp_secret` / `encrypted_custom_fields` | AES-GCM(entryKey, AAD) | 否 |
| **整库保护** | `title` / `username` / `url` / `tags` / `category` / `revision` / 时间戳 | SQLCipher 页加密 | 是 |

规则：

1. **禁止**把 `title` / `username` / `url` / `tags` 迁入字段级密文，除非同时提供盲索引；否则列表、搜索、排序全部失效。
2. 这四列属于「整库保护」，其威胁模型是「磁盘/备份泄露」；**不承诺**在 `security.db` 被解锁后仍保密。
3. 重建本机库（I9-key-hierarchy-spec §4.7）时，这四列必须能从 `entries/*.enc` 的 `EntryBlobV2` 完整恢复。
4. `encrypted_custom_fields` 的 JSON 结构固定为：

```jsonc
[
  { "name": "PIN", "valueEncrypted": "b64", "isProtected": true },
  { "name": "备注", "value": "明文", "isProtected": false }
]
```

未受保护项的 `value` 在整库加密内，属于「整库保护」；受保护项的 `valueEncrypted` 属于「字段级密文」。

### 2.4 AAD

#### 2.4.1 条目文件 AAD

```
"i9:v1:entry-file" ‖ 0x00 ‖ id ‖ 0x00 ‖ category ‖ 0x00
‖ be64(revision) ‖ be64(dekVersion)
```

- 编码规则与 `build_slot_aad` 一致：ASCII 文本段、`0x00` 分隔、`u64` 大端。
- `dekVersion` 在 v1 文件不存在时按 `0` 编码（仅用于拒绝旧文件，不参与新写入）。
- **能力边界（写进注释）**：AAD 只能阻止「改字段后仍通过认证」，**不能阻止整块旧文件重放**。重放由仓库 `manifest` + 本机 `anchor` 检测（见 sync-crypto-spec）。

#### 2.4.2 字段 AAD

```
"i9:v1:field" ‖ 0x00 ‖ entryId ‖ 0x00 ‖ fieldName
```

`fieldName` 取值固定为：

| 字段 | `fieldName` |
|---|---|
| 密码 | `password` |
| 备注 | `notes` |
| TOTP secret | `totp` |
| 自定义字段 | `custom:{name}` |
| SSH 私钥 | `ssh-private-key` |

作用：

- 阻止同一 `entryKey` 下把密文在列之间搬家（修复 E5）；
- 阻止自定义字段之间互换；
- `entryId` 进入 AAD，使「把 A 条目的密文塞进 B 条目」在解密阶段就失败，而不是依赖外层 `id` 校验。

### 2.5 `EntryFile` 可选字段与完整性

| 项 | 规定 |
|---|---|
| 文件字节 | `serde_json::to_string_pretty`，UTF-8，末尾无强制换行 |
| 原子写 | 临时文件 + rename；临时文件后缀 `.tmp`，失败时删除 |
| `encrypted_blob` | 单一 b64 串，长度固定 `12 + ctLen + 16` |
| 认证 | GCM tag 覆盖 AAD 与密文 |
| 仓库级完整性 | 由 `.vault-meta.manifest` 的 sha256 覆盖（见 sync-crypto-spec） |
| 早退优化 | 仅当 `existing.revision == snapshot.revision && existing.dekVersion == 当前` 时跳过写；否则必须重写 |

---

## 3. 接口

位置：`src-tauri/crates/assistant-crypto/src/vault_crypto/mod.rs`

```rust
pub fn derive_entry_key(dek: &Dek, entry_id: &str) -> PerEntryKey;       // v1.5
pub fn derive_legacy_entry_key(master_key: &MasterKey, entry_id: &str) -> PerEntryKey; // 仅测试/迁移诊断，不用于新写入

pub fn encrypt_entry_blob(
    key: &PerEntryKey,
    blob: &EntryBlobV2,
    aad: &[u8],
) -> Result<String, VaultError>;

pub fn decrypt_entry_blob(
    key: &PerEntryKey,
    encrypted_b64: &str,
    aad: &[u8],
) -> Result<EntryBlobV2, VaultError>;

pub fn encrypt_field(
    key: &PerEntryKey,
    plaintext: &VaultPlaintext,
    field_name: &str,
    aad: &[u8],
) -> Result<String, VaultError>;

pub fn decrypt_field(
    key: &PerEntryKey,
    encrypted_b64: &str,
    field_name: &str,
    aad: &[u8],
) -> Result<VaultPlaintext, VaultError>;
```

位置：`src-tauri/crates/assistant-vault/src/vault_file/mod.rs`

```rust
pub fn export_entry(&self, dek: &Dek, snapshot: &VaultEntrySnapshot) -> Result<(), SyncError>;
pub fn decode_entry_file(&self, dek: &Dek, content: &str) -> Result<ImportedEntry, SyncError>;
pub fn entry_file_aad(id: &str, category: &str, revision: i64, dek_version: u32) -> Vec<u8>;
```

位置：`src-tauri/crates/assistant-vault/src/security_db/entries/field_crypto.rs`

```rust
pub(super) fn encrypt_opt(
    value: Option<&str>, key: &PerEntryKey, field: &str, entry_id: &str, algo: CipherAlgorithm,
) -> Result<Option<String>, VaultError>;

pub(super) fn decrypt_opt(
    value: Option<&str>, key: &PerEntryKey, field: &str, entry_id: &str,
) -> Result<Option<String>, VaultError>;
```

约束：

1. `decrypt_opt` **禁止** `unwrap_or_default`；UTF-8 失败 → `VaultError::EntryCorrupted`。
2. `decrypt_custom_fields` 遇到任一受保护项失败 → 整条返回错误，**禁止** `.ok()?` 丢弃该项。
3. `decrypt_entry`（整条解密）失败时返回 `EntryCorrupted`，并在错误消息中只带 `id`，不带任何字段值。
4. 所有 AAD 构造器必须能被单测逐字节断言（见 §7）。

---

## 4. 文件改动清单

| 文件 | 改动 |
|---|---|
| `src-tauri/crates/assistant-crypto/src/vault_crypto/types.rs` | 新增 `Dek`（见 key-hierarchy-spec）；`PerEntryKey` 保持不变 |
| `src-tauri/crates/assistant-crypto/src/vault_crypto/mod.rs` | 新增 `encrypt_entry_blob` / `decrypt_entry_blob`；`encrypt_field` / `decrypt_field` 增加 `field_name` 与 `aad` 参数；删除 v1 `derive_entry_key(MasterKey)` 的生产调用 |
| `src-tauri/crates/assistant-vault/src/vault_file/types.rs` | `EntryFile` → `EntryFileV2`；`EntryBlob` → `EntryBlobV2`；新增 `TotpBlob`；删除 `totp_secret/digits/period` 三字段 |
| `src-tauri/crates/assistant-vault/src/vault_file/mod.rs` | `export_entry` 去掉 `force`，改为 revision+dekVersion 比较；`decode_entry_file` 严格校验 `version/id/cipher`；新增 `entry_file_aad`；`ImportedEntry::into_import` 适配 `TotpBlob` |
| `src-tauri/crates/assistant-vault/src/security_db/entries/field_crypto.rs` | 全量重写：字段名与 AAD、错误不吞、`totp` 结构编解码 |
| `src-tauri/crates/assistant-vault/src/security_db/entries/write.rs` | 4 处字段加密改为新签名；`create_entry_in` 写入 `TotpBlob` |
| `src-tauri/crates/assistant-vault/src/security_db/entries/import.rs` | 4 处字段加密改为新签名；保留 `revision` 合并逻辑 |
| `src-tauri/crates/assistant-vault/src/security_db/entries/snapshot.rs` | 解密改为新签名；TOTP 从 `TotpBlob` 还原 |
| `src-tauri/crates/assistant-vault/src/security_db/entries/deleted.rs` | 同上 |
| `src-tauri/crates/assistant-vault/src/security_db/crypto_ops.rs` | 6 处解密改为新签名；`decrypt_entry` 不再用 `.ok()?` 吞错 |
| `src-tauri/crates/assistant-vault/src/security_db/import_export.rs` | `export_entries` 只做「解密 → 明文 DTO」，不改变加密层；导入路径必须走 `create_entry_tx`（§5.2） |
| `src-tauri/crates/assistant-vault/src/security_db/schema.rs` | 仅加注释说明 §2.3 的字段分类；DDL 不变 |
| `src-tauri/crates/assistant-vault/src/types/mod.rs` | `VaultEntrySnapshot` 增加 `dek_version`（可选）；`Debug` 继续脱敏 |
| `src-tauri/crates/assistant-vault/src/dto/totp.rs` | `payload_from_input` 改为生成 `TotpBlob` |
| `src-tauri/crates/assistant-vault/src/sync_engine/*.rs` | 调用点跟随 `export_entry` / `decode_entry_file` 签名变化 |
| `src-tauri/crates/assistant-vault/src/security_db/rekey.rs` | 全量重加密路径改用 `encrypt_entry_blob` / `decrypt_entry_blob`，并重算 revision |

---

## 5. 一致性与错误语义

### 5.1 两份副本的权威关系

| 副本 | 角色 | 丢失后果 |
|---|---|---|
| `entries/*.enc` | **权威源**（同步/恢复/重建） | 条目永久丢失 |
| `security.db` | 本机索引/缓存（可重建） | 重建后从 `.enc` 恢复 |

规则：

1. 任何写操作必须先成功写 `.enc`，再提交 DB 事务；或反之，但必须保证崩溃后能通过 `.enc` 重建。
2. `vault_entries` 的 `title/username/url/tags` 必须能从 `EntryBlobV2` 完整恢复。
3. 删除条目：先删 DB 行（软删），再删 `.enc`；恢复时反向。
4. **禁止**存在只写 DB、不写 `.enc` 的生产路径（当前 `import_entries` 是唯一例外，见 §5.2）。

### 5.2 导入路径

`vault_import` 当前只调用 `state.import_entries`，不写 `.enc`、不提交 git（E6）。规格要求：

```
1. 解析文件 → Vec<VaultEntryCreate>
2. 对每条调用 create_entry_tx（含 export 回调）或等价的事务化导入
3. 全部成功后调用 sync_engine.export_all_entries + finalize("vault: import")
4. 任一条失败 → 回滚本批（或明确报告部分成功，且已成功的已写 .enc）
```

禁止「只写库」的导入。

### 5.3 错误码

| 场景 | 错误码 |
|---|---|
| 条目文件版本不支持 | `UnsupportedEntryFileVersion` |
| `id` 与文件名不符、`revision < 0`、`cipher` 未知 | `VaultCorrupted` |
| GCM/AAD 失败（字段或 blob） | `EntryCorrupted` |
| UTF-8 非法 | `EntryCorrupted` |
| 自定义字段 JSON 结构非法 | `EntryCorrupted` |

`EntryCorrupted` 只携带 `id`，不得携带任何明文字段值。新增错误码需同步 `VaultErrorCode`、`convert.rs`、`vaultError.ts`。

---

## 6. 兼容与迁移

1. v1 的 `EntryFile`（无 `version`/`dekVersion`/`cipher`，`encryptedBlob` 首字节带算法）**不兼容读取**。判定为 `UnsupportedEntryFileVersion`。
2. 与 [I9-vault-capability-gating-spec.md](./I9-vault-capability-gating-spec.md) §3.2 一致：不写迁移器，旧库整体拒绝并提示重置。
3. 本规格的 `EntryBlobV2` 是**仓库格式**变更，若将来需要跨版本读取，必须新增 `schema` 分支并显式版本化，不得静默兼容。

---

## 7. 测试

| 编号 | 用例 | 断言 |
|---|---|---|
| T-E1 | `EntryFileV2` 往返（100 条随机 blob） | 解密后字段完全一致；`totp.algorithm/digits/period` 保留 |
| T-E2 | 篡改 `EntryFileV2.revision` 后解密 | `EntryCorrupted`（AAD 不符） |
| T-E3 | 篡改 `EntryFileV2.category` 后解密 | `EntryCorrupted` |
| T-E4 | 把 A 条目的 `encryptedBlob` 放进 B 条目的文件 | `EntryCorrupted`（AAD 的 `id` 不符） |
| T-E5 | 把 `encrypted_password` 密文写入 `encrypted_notes` | `EntryCorrupted`（字段 AAD 不符） |
| T-E6 | 受保护自定义字段密文互换 | `EntryCorrupted` |
| T-E7 | 受保护字段密文被截断 | `EntryCorrupted`，不得返回空串 |
| T-E8 | 非 UTF-8 明文 | `EntryCorrupted`，不得 `unwrap_or_default` |
| T-E9 | import 后立即检查 `entries/*.enc` | 每条都有文件；revision 与 DB 一致 |
| T-E10 | `cipher` 与 blob 实际算法不一致 | `VaultCorrupted` |
| T-E11 | `version=1` 的 `EntryFile` | `UnsupportedEntryFileVersion`，文件不被修改 |
| T-E12 | TOTP 三参数经「DB → 仓库 → 重建 DB」往返 | 算法/位数/周期全部保持 |

测试落点：

- `src-tauri/crates/assistant-vault/tests/vault_file.rs`（已存在）扩展 `EntryFileV2` / AAD / 版本判定用例；
- `src-tauri/crates/assistant-vault/tests/vault_crud.rs`、`security_db.rs` 扩展字段级 AAD 与静默失败用例；
- 新增 `tests/entry_cipher.rs` 放 T-E1–T-E12 的集中用例，避免与现有文件职责混淆。

补充断言：`vault_entries` 的 4 个明文字段在 `EntryBlobV2` 中均有对应值；重建库后列表搜索行为不变。

---

## 8. 禁止事项

1. **禁止**在密文外新增任何参与业务判定的字段而不加入 AAD。
2. **禁止**在解密失败时返回空串、空数组或跳过该字段。
3. **禁止**把 `title/username/url/tags` 改为字段级密文而不提供盲索引。
4. **禁止**新增只写 DB、不写 `entries/*.enc` 的导入/创建路径。
5. **禁止**在 `EntryBlobV2` 里继续使用三个分散的 TOTP 字段。
6. **禁止**在错误消息、日志、审计 detail 中出现任何明文字段值。

# I9 — 密码箱密钥体系 v1.5 实施规格

> **本文档职责**：把 [I9-vault-key-hierarchy-design.md](./I9-vault-key-hierarchy-design.md) 的设计决策落成**可实施规格**——精确到每个文件的改动、每个类型的字段布局、每个函数的签名与行为约束、每个错误码的映射、每个阶段的验收标准。是 v1.5 密钥体系**类型定义、字节布局、接口签名、错误契约**的唯一权威源。
> **架构决策记录**（为什么这样设计、否决了什么、代价记账）见设计文档，本文不重复论证。
> **覆盖缺口**：设计文档在 **22 处**只给出"要做什么"而未给出"怎么做/长什么样"（见 §0），其中 9 处是**设计本身需要修正**的内部冲突或归属缺失（见 [附录 C](#附录-c与设计文档的差异修正记录)）。本文逐条补齐并记录修正。**例外：G22（同步层）本期只登记缺口、不补齐**——它牵涉跨设备契约，需先决策再实施，见 §4.11。

---

## 0. 覆盖缺口清单

设计文档 §2–§7 覆盖了密钥层次、操作语义、威胁矩阵与阶段划分，但以下 21 项**不足以直接开工**。本规格逐项补齐：

| 编号 | 缺口 | 设计文档现状 | 本规格补齐位置 |
|---|---|---|---|
| **G1** | 密钥类型未定义 | 只出现 `Dek` / `KEK` / `dbKey` 名字，无字段布局、无 Zeroize 策略、无所在模块 | §2.1–2.2 |
| **G2** | 错误码未落到枚举 | 只给 6 个语义名（`UnsupportedVaultVersion` 等），无 `VaultError` 变体签名、无 `VaultErrorCode` 枚举项、无 `convert.rs` 映射、无前端数组、无 retryable 判定 | §7 |
| **G3** | AAD 字节编码未定 | 只给 `‖ 0x00 ‖` 拼接示意，未定 `be64` 实现与各段长度/字符集约束 | §3.6 |
| **G4** | "规范化 JSON" 未定 | 只写"除本字段外全部字段的规范化 JSON"，未定键序、分隔符、数字格式 | §3.7 |
| **G5** | 纸面编码未给字母表与 CRC 参数 | 只给长度 40、5 组、25 字节构成，未给 Crockford 字母表、CRC32C 多项式/初值/反射、解码校验顺序 | §3.8 |
| **G6** | manifest 未定义 | 只写"每条目 `sha256` + `dekVersion`"，未定 sha256 覆盖对象（密文文件字节 vs 明文）、排序、`count` 语义、计算时机 | §2.3、§4.2 |
| **G7** | anchor 未定义 | 只写 `{ metaHash, dekVersion, revision }`，未定存储形态、`metaHash` 算法、`revision` 递增时机、校验顺序 | §4.6 |
| **G8** | `vaultId` / `dekVersion` 取值规则未定 | 无格式、无初始值；§2.3 示例写 `dekVersion: 3`，§9.1 I1 却断言 `dekVersion=1` | §1.4、附录 C-1 |
| **G9** | 命令层只给名字 | 无签名、无参数、无返回 DTO、无前置条件、无错误集合 | §9 |
| **G10** | 新命令的 DTO 未定义 | `vault_export_recovery_key` / `vault_recover_from_paper` / `vault_rotate_dek` 返回什么完全未定 | §9.2 |
| **G11** | 前端接口未定义 | 只给方法名 `exportRecoveryKey` / `recoverFromPaper` / `rotateDek`，无参数/返回类型、无错误码联合类型扩展、无 i18n key | §10 |
| **G12** | 生物识别细节未定 | 52B 的 20/32 切分给了，但 `fingerprint` 存哪个 key id、重包失败如何标记、探针读的 SQL 未定 | §5 |
| **G13** | `kdf_iter=1` 的验证方法未给 | §8 P0-1 写"先用一条测试确认 raw key 下该 pragma 是否为 no-op"，未给测试形态与判定 | §11.1 |
| **G14** | 旧库判定未精确 | 未定"`version` 缺失且无 `crypto.masterKeySalt`"算什么 | §4.3 |
| **G15** | 审计 JSONL 未定 | 未定路径、命名、轮转、写入原子性、"下次解锁补录"的具体去重键 | §8 |
| **G16** | 阶段验收标准未给 | §8 只有"可交付性"一句话，无可执行断言 | §11 |
| **G17** | 设计内部冲突 | §2.6 出现 `Corrupted`，但现有 `VaultErrorCode` 无此项；§9.2 I4 断言 `InvalidRecoveryKey`，§2.7 却是 `InvalidRecoveryKeyOrMetaTampered` | 附录 C-2 |
| **G18** | `recoveryBackup` 写入时机与查询接口未给 | 只写"写 `recoveryBackup` 标记"，未定何时写、谁写、"是否已备份"如何查 | §6.4 |
| **G19** | `slot.id` 递增的持久化规则未定 | 只写 `pwd-3` → `pwd-4`，未定首个 id、如何解析出下一个 | §1.4 |
| **G20** | 编排层归属未定 | §3.2 / §3.3 的流程跨 `vault_file` 与 `security_db` 两层，但 §6 改动清单里没有任何一个文件负责编排；且 §6.3 完全漏掉 `cmd/vault/lifecycle.rs`（`vault_init` / `vault_unlock` 的签名必须变） | §4.1、§9.1 |
| **G21** | 重置仓库命令未定义 | §5 第 3 步"重置仓库"无命令名、无确认机制、无审计事件 | §9.3 |
| **G22** | **同步层未覆盖（未补齐）** | 设计 §6.2 只用一行列了 5 个文件，未定 v1.5 下 `.vault-meta` 的写入方、pull 后 anchor 与 manifest 如何重建、`.vault-meta` 冲突如何处置、远端 `dekVersion` 超前如何应对 | §4.11（**仅缺口登记**，实施前必须先补成规格） |

---

## 1. 实施总览

### 1.1 分层与搭建顺序

```
Step 0  值类型            Dek · Kek · DbKey · MetaKey · VaultId · DekVersion · VaultMetaFileV2 · Anchor · RecoverySession
Step 1  密码学原语        derive_kek · derive_db_key · derive_entry_key · derive_meta_key · seal/open_dek · AAD · integrity · 纸面编解码
Step 2  仓库文件层        .vault-meta v2 严格解析 · 原子写 · manifest · integrity 校验
Step 3  本机库层          schema · meta · init · unlock · change_password · rekey · anchor
Step 4  编排层            initialize / unlock / change_password / rotate_dek 的跨层流程
Step 5  生物识别层        52B payload · fingerprint · 探针读 · 自动重包
Step 6  恢复通道层        RecoverySession · export_all · 删除 unlock_with_recovery_key
Step 7  错误码层          VaultError 变体 · VaultErrorCode · convert 映射 · 前端数组
Step 8  审计层            AuditEvent 扩展 · JSONL 双写 · 补录
Step 9  命令层与 DTO      命令增删 · DTO 定义 · 前置条件
Step 10 前端层            service · types · 三个组件 · i18n
```

### 1.2 三层性质

| 性质 | Steps | 说明 |
|---|---|---|
| **词汇层** | 0–1 | 定义"密钥长什么样、密文长什么样"。零 IO。没有它写不出任何签名 |
| **数据层** | 2–3 | 唯一读写磁盘的地方（`.vault-meta` 与 `security.db`） |
| **编排层** | 4 | 把两层串成"解锁/改密/轮换"这三个不可分割的事务。**没有它，跨层顺序会散落在命令层里各写一遍**（当前 v1 就是这样） |
| **能力层** | 5–6 | 生物识别与恢复通道。两者都绕开口令，必须显式裁剪权限 |
| **契约层** | 7–8 | 错误码与审计。前端与运维唯一能看到的东西 |
| **连接层** | 9–10 | 命令与前端 |

### 1.3 阶段 ↔ Step 映射

| 阶段（设计 §8） | 本文 Steps | 不可发布窗口 |
|---|---|---|
| **P0-1** | 1.5（仅 `kdf_iter`）+ 1.9（PRAGMA 去重） | — |
| **P0-2** | 0、1（除纸面）、2、3、4、7、9（除恢复命令）、10（除纸面 UI） | **P0-2 与 P0-3 之间没有恢复通道，不可发布** |
| **P0-3** | 1.8（纸面编解码）+ 6 + 9（恢复命令）+ 10（纸面 UI） | — |
| **P1-1** | 3.3（manifest）+ 4.6（anchor） | — |
| **P1-2** | 5 | — |
| **P1-3** | 4.5（rotate_dek）+ 5.6（强制重注册） | — |

### 1.4 取值规则（补 G8 / G19）

| 项 | 规则 | 依据 |
|---|---|---|
| `vaultId` | `Uuid::new_v4().simple().to_string()` → **32 个小写 hex 字符，无连字符**；首次初始化生成，`rotate_dek` 不变 | 设计 §10 D10；`uuid` 已在 `assistant-vault` 依赖中 |
| `dekVersion` | **初值 1**，`rotate_dek` 后 `+1`，单调不减，`>= 1` | 设计 §9.1 I1；§2.3 示例中的 `3` 只是示意，见附录 C-1 |
| `slot.id` | `pwd-{n}`，`n` 从 **1** 开始。下一次改密时 `n = 当前 slot.id 解析出的数字 + 1`；解析失败（非 `pwd-{n}` 形态）→ `VaultCorrupted` | 设计 §3.2 "`pwd-3` → `pwd-4`" |
| `slot.createdAt` | **毫秒**（`timestamp_millis`），与项目其余时间体系一致 | `security_audit_log.timestamp` 亦为毫秒 |
| `recoveryBackup.confirmedAt` | **毫秒** | 同上 |
| `manifest.count` | 恒等于 `manifest.entries.len()`；不一致 → `VaultMetaIntegrityFailed` | 设计 §9.2 I6 |
| `anchor.revision` | 从 **0** 起，每成功写一次 `.vault-meta` 后 `+1` | 设计 §2.8 L3 |

---

## 2. Step 0 — 值类型层

> **建什么**：v1.5 全部密钥与文件结构的类型定义。
> **为什么先建**：上面每一层都引用这些类型；没有 `Dek` 就写不出 `derive_entry_key` 的签名，没有 `VaultMetaFileV2` 就写不出严格解析。
> **建完能干什么**：可以写 `VaultCrypto` 的新原语与 `.vault-meta` v2 的解析。

### 2.1 `Dek`（新增，20 字节）

位置：`src-tauri/crates/assistant-crypto/src/vault_crypto/types.rs`（与 `MasterKey` 同文件，但**必须是独立 newtype**——`MasterKey` 固定 `[u8; 32]`，`Dek` 是 `[u8; 20]`，不可复用）。

```rust
pub struct Dek([u8; 20]);

impl Dek {
    pub fn new(bytes: [u8; 20]) -> Self;
    pub fn generate() -> Self;                      // rand::thread_rng().gen()
    pub fn expose(&self) -> &[u8; 20];
    pub fn to_bytes(&self) -> [u8; 20];
    pub fn ct_eq(&self, other: &Dek) -> bool;       // subtle::ConstantTimeEq
}

impl Zeroize for Dek;   // 逐字节清零
impl Drop for Dek;      // 调 zeroize
impl fmt::Debug for Dek // 输出 "Dek([REDACTED; 20 bytes])"
```

约束：

1. **禁止实现 `Clone`**。需要传递时用 `&Dek` 或 `Dek::to_bytes()` 重建。理由：多份副本会绕过 `Drop` 清零的预期覆盖点。
2. **禁止实现 `Copy` / `Display` / `Serialize`**。序列化 `dek` 只有一条合法出口：`encode_paper`。
3. `ct_eq` 用于 `fingerprint` 比对与任何密钥比较；`PartialEq` 不派生，避免被误用于密钥比较。

### 2.2 `Kek` / `DbKey` / `MetaKey`

同文件，同规格（32 字节 newtype + `Zeroize` + `Drop` + 脱敏 `Debug`，不派生 `Clone` / `Copy` / `Serialize`）：

```rust
pub struct Kek([u8; 32]);
pub struct DbKey([u8; 32]);
pub struct MetaKey([u8; 32]);
```

`EntryKey` **复用现有 `PerEntryKey`**（同为 32 字节，已实现 `Zeroize`），不新增类型。

### 2.3 `VaultMetaFileV2` 与子结构

位置：`src-tauri/crates/assistant-vault/src/vault_file/types.rs`。

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultMetaFileV2 {
    pub version: u32,                       // 恒 2
    pub vault_id: String,                   // 32 小写 hex
    pub dek_version: u32,                   // >= 1
    pub cipher: String,                     // 恒 "aes256Gcm"
    pub slots: Vec<VaultSlot>,              // 当前长度恒 1
    pub manifest: VaultManifest,
    #[serde(default)]
    pub recovery_backup: Option<RecoveryBackup>,
    pub integrity: String,                  // b64(HMAC-SHA256)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultSlot {
    pub id: String,                         // "pwd-{n}"
    #[serde(rename = "type")]
    pub slot_type: String,                  // 恒 "password"
    pub payload_kind: String,               // 恒 "dek"
    pub kdf: KdfParams,
    pub wrapped_dek: String,                // b64(48 bytes)
    pub created_at: i64,                    // 毫秒
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KdfParams {
    pub algo: String,                       // 恒 "argon2id"
    pub salt: String,                       // b64(16)
    pub m: u32,                             // 65536
    pub t: u32,                             // 3
    pub p: u32,                             // 4
    pub v: u32,                             // 19
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultManifest {
    pub count: u32,
    pub entries: Vec<ManifestEntry>,        // 按 id 字典序升序
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestEntry {
    pub id: String,
    pub sha256: String,                     // b64(32)
    pub dek_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryBackup {
    pub dek_version: u32,
    pub confirmed_at: i64,                  // 毫秒
}
```

约束：

1. `#[serde(rename = "type")]` 是必须的——`type` 是 Rust 关键字，字段名只能是 `slot_type`。
2. **`deny_unknown_fields` 不加**：未来加 slot 类型时旧版本应能读到（未知字段被忽略）；但**未知枚举值**必须拒绝（见 §4.3 第 6 步字段校验）。
3. `VaultMetaFileV2` 不含 `exportedAt`。v1 的 `exported_at`（`vault_file/types.rs:92-95`，注明"unix 秒、跨设备契约、勿改"）随 `export_vault_meta` / `write_emergency_data` 一并删除（唯一读写点 `vault_file/mod.rs:233、258、330、334、357`）。v2 的时间字段一律**毫秒**，与该注释描述的 v1 契约不冲突（v2 无跨设备导出契约）。

### 2.4 `Anchor`

位置：`src-tauri/crates/assistant-vault/src/security_db/meta.rs`（与 KV 读写同文件）。

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anchor {
    pub meta_hash: String,      // 小写 hex(SHA-256)，见 §4.6
    pub dek_version: u32,
    pub revision: u64,
}
```

**存储形态**：`vault_meta` 表是 `key/value` 两列（`security_db/schema.rs:4-7`），因此 anchor 落成**三个 KV 行**，不是三个列：

| key | value |
|---|---|
| `anchor_meta_hash` | 小写 hex |
| `anchor_dek_version` | `u32` 十进制字符串 |
| `anchor_revision` | `u64` 十进制字符串 |

> 设计 §6.1 写"新增 `anchor_meta_hash`、`anchor_dek_version`、`anchor_revision`"，字面像列；实际是 KV 键。**不改 DDL**，因此 `SCHEMA_VERSION` 保持 `2`（`schema.rs:1`）。这是本规格对设计的一处澄清（附录 C-3）。

### 2.5 `RecoverySession`

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

1. **不持有 `SecurityDbState`、不持有 `Connection`、不持有 `DbKey`**。设计 §4.5 只写了 `dek` 一个字段；仅持 `dek` 时 `export_all` 无法读磁盘，故补 `entries_dir` 与 `manifest`（均为只读、无密钥）。这是本规格对设计的补全（附录 C-4）。
2. **禁止提供** `conn()` / `db()` / `exec()` / `audit()` / `db_key()`。`db_key` 拿不到是编译期事实，不是运行时约定。
3. `PlainEntry` 定义在 `assistant-vault/src/types.rs`：
   ```rust
   pub struct PlainEntry {
       pub id: String,
       pub category: String,
       pub blob: EntryBlob,        // 复用 vault_file::types::EntryBlob
   }
   ```

---

## 3. Step 1 — 密码学原语层

> **建什么**：v1.5 的 6 个派生/包裹原语、AAD 构造器、integrity 规范化、纸面编解码。
> **为什么先建**：Step 2/3 全部调用它们；AAD 与规范化一旦定错，所有密文与签名都不可复现。
> **建完能干什么**：可以写 `.vault-meta` v2 的写入与严格解析。

位置统一：`src-tauri/crates/assistant-crypto/src/vault_crypto/mod.rs`（`assistant-security/src/vault_crypto/mod.rs` 只是 `pub use` 转发，无需改）。

### 3.1 `derive_kek`

```rust
pub fn derive_kek(password: &str, salt: &[u8], m: u32, t: u32, p: u32) -> Result<Kek, VaultError>;
```

行为：

1. `Argon2id`，`Version::V0x13`，输出 **32 字节**直接作为 `KEK`（**不经 HKDF**，设计 §2.4）。
2. `salt` 长度必须为 **16 字节**（`KDF_SALT_LEN`）；否则 `VaultError::VaultCorrupted`。
3. 参数非法（`m < 8192` / `t < 1` / `p < 1`）→ `VaultError::VaultCorrupted`。**禁止静默用默认参数**（设计 §2.3 "禁止任何静默降级"）。

### 3.2 `derive_db_key`

```rust
pub fn derive_db_key(kek: &Kek, vault_id: &str) -> DbKey;
```

`HKDF-SHA256(salt = 空, ikm = KEK, info = "i9:v1:sqlcipher:" + vault_id)`，输出 32 字节。
`vault_id` 为空 → `panic!`（编程错误，不是用户输入）；调用点已在 §3.1 校验过格式。

### 3.3 `derive_entry_key`

```rust
pub fn derive_entry_key(dek: &Dek, entry_id: &str) -> PerEntryKey;
```

`HKDF-SHA256(salt = 空, ikm = dek, info = "i9:v1:entry:" + entry_id)`，输出 32 字节。

> **与 v1 的差别**：v1 的 `derive_entry_key(master_key, entry_id)` 用 `entry_id` 裸做 `info`（`assistant-crypto/src/vault_crypto/mod.rs:64-70`，`hkdf.expand(entry_id.as_bytes(), ..)`），**无 `i9:v1:entry:` 前缀**。v1.5 必须加前缀——这是**密文不兼容**的第二个来源（第一个是 `ikm` 从 32B 口令派生变 20B 随机），所以旧库无法"顺手兼容读"，与设计 §5 的不迁移决策自洽。

### 3.4 `derive_meta_key`

```rust
pub fn derive_meta_key(dek: &Dek) -> MetaKey;
```

`HKDF-SHA256(salt = 空, ikm = dek, info = "i9:v1:meta-integrity")`，输出 32 字节。

### 3.5 `seal_dek` / `open_dek`

```rust
pub fn seal_dek(kek: &Kek, dek: &Dek, aad: &[u8]) -> Result<Vec<u8>, VaultError>;
pub fn open_dek(kek: &Kek, wrapped: &[u8], aad: &[u8]) -> Result<Dek, VaultError>;
```

字节布局（`wrappedDek` 解码后 **48 字节**，b64 后 **64 字符**）：

```
nonce(12) ‖ ciphertext(20) ‖ tag(16)
```

约束：

1. `seal_dek` 每次随机 12 字节 nonce；`open_dek` 输入长度 ≠ 48 → `VaultError::VaultCorrupted`。
2. GCM 解密失败（含 AAD 不符）→ `VaultError::InvalidPassword`。**这是设计 §2.6 第 3 步的语义：口令错与密文损坏在此不可区分。**
3. 禁止把 `nonce` 与 `tag` 拆成独立字段存 meta——`wrappedDek` 是单一 b64 串（设计 §2.3）。

### 3.6 AAD 构造器（补 G3）

```rust
pub fn build_slot_aad(
    payload_kind: &str,
    slot_type: &str,
    slot_id: &str,
    dek_version: u32,
) -> Vec<u8>;
```

精确字节序列：

```
b"i9:v1:slot"           // 11 字节字面量
‖ 0x00
‖ payload_kind.as_bytes()   // "dek"
‖ 0x00
‖ slot_type.as_bytes()      // "password"
‖ 0x00
‖ slot_id.as_bytes()        // "pwd-3"
‖ 0x00
‖ dek_version.to_be_bytes() // u64 大端，8 字节
```

约束：

1. 每个文本段必须**非空**且**不含 `0x00`**、必须是 ASCII 可打印字符；违反 → `panic!`（编程错误）。
2. 前缀字面量**照抄 `i9:v1:slot`**，不要"顺手改成 v2"（设计 §2.4 同名警告同样适用）。
3. `dek_version` 用 `u64` 而非 `u32` 编码——给未来留位，解码端不需要读回（AAD 只做比对）。

**必须写进函数注释的能力边界**（设计 §2.5）：当 `dek` 未变（仅改密）时，新旧 meta 的 AAD 完全一致，整块旧 meta 重放**无法被 AAD 拦截**。这是 C-1 的直接体现。

### 3.7 integrity 规范化（补 G4）

**不使用 `serde_json::Value` 做规范化**（键序依赖 `serde_json` 的 `preserve_order` feature，当前 workspace 未启用，但未来任一 crate 打开该 feature 就会静默改变签名——不可接受）。

改为**专用镜像结构体**，字段顺序固定、无 `Option` 以外的动态性：

```rust
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VaultMetaIntegrityView<'a> {
    version: u32,
    vault_id: &'a str,
    dek_version: u32,
    cipher: &'a str,
    slots: &'a [VaultSlot],
    manifest: &'a VaultManifest,
    recovery_backup: Option<&'a RecoveryBackup>,
}

pub fn canonical_meta_bytes(meta: &VaultMetaFileV2) -> Result<Vec<u8>, VaultError> {
    serde_json::to_vec(&VaultMetaIntegrityView { .. })
}
```

规则：

| 项 | 规定 |
|---|---|
| 覆盖范围 | 除 `integrity` 外的**全部**字段（与设计 §2.8 L1 一致） |
| 键序 | 由 `VaultMetaIntegrityView` 的**字段声明顺序**决定（serde 对 struct 稳定输出声明序） |
| 分隔符 | `serde_json::to_vec` 的紧凑格式（无空格、无换行） |
| 数字 | 全部为整数（`u32` / `i64`），无浮点 |
| 缺省 `recoveryBackup` | 输出 `"recoveryBackup":null`（`Option::None` 的标准序列化），**不省略键** |
| 签名值 | `b64(HMAC-SHA256(metaKey, canonical_meta_bytes))` |

**必须配一条黄金向量测试**（§11.2 T-G1）：固定一组字段 → 断言 `canonical_meta_bytes` 的输出字节与硬编码期望完全一致。这条测试是"跨实现可复现"的唯一保障。

> 与 v1 的差别：v1 用 `serde_json::to_vec(&meta_for_hmac)` 把 `integrity` 置空后整体序列化（`vault_file/mod.rs:439-444`），**结构体里含 `integrity: ""`**。v2 直接删掉该字段，因此同一组数据在 v1/v2 下签名不同——符合"不兼容"决策。

### 3.8 纸面编解码（补 G5）

位置：`src-tauri/crates/assistant-vault/src/recovery_key/mod.rs`。

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

**字节布局**（25 字节 → 200 bit → 40 字符，恰好整除）：

```
version(1) = 0x02 ‖ dek(20) ‖ crc32c(4, 大端)
```

**CRC32C 参数**（`crc::CRC_32_ISCSI`，即 Castagnoli）：

| 项 | 值 |
|---|---|
| 多项式（反射） | `0x82F63B78` |
| init | `0xFFFFFFFF` |
| xorout | `0xFFFFFFFF` |
| 反射输入/输出 | 是 |
| 计算范围 | 前 **21** 字节（`version ‖ dek`） |

依赖：`assistant-vault/Cargo.toml` 新增 `crc = "3"`，workspace `Cargo.toml` 同步新增。若不愿增依赖，可手写 256 项查表实现（参数同上），但**禁止改用非 CRC32C 的多项式**（否则与设计 §4.4 的"2^-32 漏检"论证不符）。

**归一化规则**（顺序不可变，逐条实现）：

1. 全角 `０-９`、`Ａ-Ｚ` → 半角（`U+FF10..U+FF19` → `0..9`，`U+FF21..U+FF3A` → `A..Z`）
2. 转大写
3. 删除 `-`、空格、`\t`、`\r`、`\n`、全角空格 `U+3000`
4. `O` → `0`
5. `I` / `L` → `1`
6. `U` → `V`

**解码校验顺序**（顺序决定错误码，不可调换）：

| 步骤 | 失败错误码 |
|---|---|
| ① 归一化后长度 ≠ 40 | `InvalidRecoveryKeyFormat` |
| ② 含字母表外字符 | `InvalidRecoveryKeyFormat` |
| ③ base32 解码后长度 ≠ 25 | `InvalidRecoveryKeyFormat` |
| ④ `bytes[0] != 0x02` | `InvalidRecoveryKeyFormat` |
| ⑤ CRC32C 不符 | `InvalidRecoveryKeyChecksum` |
| ⑥ 通过 | 返回 `Dek` |

**`locate_checksum_error` 算法**（纠错定位，设计 §2.7 第 1 步）：

1. 对归一化后的每个位置 `i ∈ 0..40`，逐个尝试字母表中除原字符外的 **31** 个替换值；
2. 对每个候选执行 ②–⑤；
3. **恰好一个**候选通过 CRC → 返回 `(i / 8 + 1, i % 8 + 1)`；
4. 零个或多个候选通过 → 返回 `None`（UI 只提示"校验失败"，不猜位置）。

约束：

1. `locate_checksum_error` **不得触达任何口令相关代码路径**（设计 §9.1 T5b）。它只做 base32 + CRC32C。
2. 复杂度上界 40 × 31 = 1240 次 CRC，单次 < 1µs，无需优化。
3. `decode_paper` 返回 `Dek` 时**不校验 HMAC**——HMAC 校验在 §6.3 的恢复流程里（设计 §2.7 第 4 步），因为需要读 `.vault-meta`。

### 3.9 常量总表

见 [附录 A](#附录-a常量与字面量总表)。实现时所有魔法数字必须引用常量，禁止字面量散落。

---

## 4. Step 2 / 3 / 4 — 仓库文件层、本机库层、编排层

### 4.1 编排层归属（补 G20）

设计 §3.2 / §3.3 的流程跨 `VaultFileManager`（`.vault-meta`）与 `SecurityDbState`（`security.db`）两层，但设计 §6 的改动清单里**没有任何文件负责编排**；且 §6.3 完全漏掉 `src-tauri/src/cmd/vault/lifecycle.rs`——`vault_init` / `vault_unlock` 现在直接调 `state.init(password)` / `state.unlock(password)`（`lifecycle.rs:10、22、89`），签名必须变。

**新增模块**：`src-tauri/crates/assistant-vault/src/lifecycle.rs`（在 `lib.rs` 加 `pub mod lifecycle;`）。

```rust
pub struct VaultPaths<'a> {
    pub repo: &'a VaultFileManager,
    pub state: &'a SecurityDbState,
}

pub fn initialize(
    paths: VaultPaths<'_>,
    password: &str,
    cipher: CipherProvider,
) -> Result<VaultMetaFileV2, VaultError>;

pub fn unlock(
    paths: VaultPaths<'_>,
    password: &str,
) -> Result<VaultMetaFileV2, VaultError>;

pub fn change_password(
    paths: VaultPaths<'_>,
    current: &str,
    new: &str,
) -> Result<VaultMetaFileV2, VaultError>;

pub fn rotate_dek(
    paths: VaultPaths<'_>,
) -> Result<(VaultMetaFileV2, String), VaultError>;   // (新 meta, 新纸面 40 字符)
```

理由（必须写进模块注释）：

1. **不可分割性**：改密的"`PRAGMA rekey` → 原子写 meta → 更新 anchor"三步跨越两层，任何一层单独暴露都会让命令层重复实现顺序（当前 `cmd/vault/entries.rs:231` 已经在命令层拼这套顺序了）。
2. **可测性**：集成测试 I2/I3/I4 直接调这 4 个函数，不需要构造 Tauri `AppHandle`。
3. 命令层退化为薄封装（§9）。

> 这是本规格**新增**的结构（设计未列）。若不采纳，替代方案是把 `VaultFileManager` 传入 `SecurityDbState::change_master_password`——但那样 `security_db` 会依赖 `vault_file`，方向相反，更差。

### 4.2 `VaultFileManager` 改动

文件：`src-tauri/crates/assistant-vault/src/vault_file/mod.rs`。

| 函数 | 处理 |
|---|---|
| `read_vault_meta`（`:260-263`） | **改写**为 `read_vault_meta_v2`（严格解析，见 §4.3）。原函数删除 |
| `export_vault_meta`（`:213-258`） | **删除**（v1 跨设备导出契约，设计 §6.2） |
| `read_emergency_data`（`:312-318`） | **删除** |
| `write_emergency_data`（`:320-346`） | **删除**（含 `.unwrap_or(VAULT_META_VERSION)` 静默沿用版本号，`:330`） |
| `clear_emergency_data`（`:347-366`） | **删除** |
| `rewrite_vault_meta_with_new_key`（`:367-425`） | **删除** |
| `verify_vault_meta_integrity`（`:265-308`） | **改写**签名为 `verify_meta_integrity(&self, meta_key: &MetaKey) -> Result<(), VaultError>`；保留 `IntegrityResult` 包装版本供命令层 |
| `write_entry_file` / `import_entry` | `derive_entry_key(master_key, id)` → `derive_entry_key(dek, id)`（2 处） |
| `write_atomically` | 保留不动（临时文件 + rename 已是原子写） |
| `compute_vault_meta_hmac` / `sign_vault_meta`（`:427-445`） | **删除**，由 §3.7 的 `canonical_meta_bytes` + HMAC 取代 |
| `VAULT_META_VERSION`（`:27`） | **删除**（v2 的版本号来自 `meta.version` 字段，不再有常量） |

新增：

```rust
impl VaultFileManager {
    pub fn read_vault_meta_v2(&self) -> Result<VaultMetaFileV2, VaultError>;
    pub fn write_vault_meta_v2(&self, meta: &mut VaultMetaFileV2, meta_key: &MetaKey)
        -> Result<(), VaultError>;
    pub fn compute_manifest(&self, dek_version: u32) -> Result<VaultManifest, VaultError>;
    pub fn meta_file_hash(&self) -> Result<String, VaultError>;   // 小写 hex(SHA-256(文件字节))
    pub fn verify_manifest(&self) -> Result<(), VaultError>;     // 比对 manifest 与实际 .enc
    pub fn entry_file_path(&self, id: &str) -> PathBuf;           // 已有，保留
}
```

**`compute_manifest` 的精确语义（补 G6）**：

| 项 | 规定 |
|---|---|
| sha256 覆盖对象 | `entries/{id}.enc` 的**完整文件字节**（即 `write_atomically` 写入的 `to_string_pretty` 文本，含缩进与换行）——**不是**明文 `EntryBlob`，也不是 `encrypted_blob` 字段值 |
| 编码 | `b64`（与项目其余字段一致，不用 hex） |
| 排序 | `entries` 按 `id` **字典序升序**（`String::cmp`，字节序）；保证同一仓库在任意机器上产生同一 manifest |
| `count` | `= entries.len()` |
| `dek_version`（条目级） | 取当前 meta 的 `dekVersion`（`rotate_dek` 后全量重算） |
| 计算时机 | ① 每次条目增/删/改后（与 `write_entry_file` 同事务）；② `change_password` **不**重算（`dek` 未变）；③ `rotate_dek` 全量重算 |
| 缺失 `.enc` | 该条目从 manifest 移除（不报错）——manifest 描述"磁盘上实际有什么" |

> sha256 覆盖密文文件字节而非明文，使 manifest 能检测"文件被替换/截断"，但不能检测"密文被正确重加密成另一个合法密文"。后者由 `integrity`（meta 的 HMAC）间接约束——`dekVersion` 与 `manifest` 都在 HMAC 覆盖范围内。

### 4.3 `read_vault_meta_v2` 严格解析（补 G14）

判定顺序**不可调换**（顺序决定错误码）：

```
1. 文件不存在 ─────────────────────▶ NotInitialized
2. 读文件失败（权限/IO） ────────────▶ Io
3. 反序列化为 serde_json::Value 失败 ▶ VaultCorrupted("unparsable .vault-meta")
4. 版本判定：
   ├─ value["version"] == 1
   ├─ 或 version 缺失且 value["crypto"]["masterKeySalt"] 存在
   ├─ 或 version 缺失且 value["emergency"] 存在
   └────────────────────────────────▶ UnsupportedVaultVersion
   ├─ version 缺失且上述特征键均不存在 ─▶ VaultCorrupted
   ├─ version 存在但 != 2 ───────────▶ VaultCorrupted
5. 反序列化为 VaultMetaFileV2 失败 ──▶ VaultCorrupted
6. 字段校验（任一失败 → VaultCorrupted）：
   ├─ vault_id 非 32 个小写 hex 字符
   ├─ dek_version == 0
   ├─ cipher != "aes256Gcm"
   ├─ slots.len() != 1
   ├─ slots[0].type != "password"
   ├─ slots[0].payloadKind != "dek"
   ├─ slots[0].kdf.algo != "argon2id"
   ├─ slots[0].kdf.salt 非 b64(16)
   ├─ slots[0].wrappedDek 非 b64(48)
   ├─ slots[0].id 非 "pwd-{n}"（n >= 1）
   ├─ manifest.count != manifest.entries.len()
   ├─ manifest.entries 未按 id 升序
   └─ recoveryBackup 存在但 dek_version > 当前 dekVersion
7. 通过 ─────────────────────────────▶ VaultMetaFileV2
```

约束：

1. **禁止任何静默降级**（设计 §2.3）。特别是：`version` 缺失**不得**当 2 处理；`slots` 为空**不得**当"未初始化"处理。
2. **不做 v1 字段映射**。第 4 步识别出 v1 后立即返回，不读任何 `crypto` / `emergency` 内容（设计 §5）。
3. **不修改任何文件**。`read_vault_meta_v2` 是纯读；`UnsupportedVaultVersion` 路径不得触碰 `.vault-meta` / `security.db` / `entries/`（设计 §9.2 I10 断言 mtime 不变）。

### 4.4 `write_vault_meta_v2`

```
1. 重算 integrity（§3.7）
2. serde_json::to_string_pretty
3. write_atomically（临时文件 + rename）
```

约束：

1. **调用方必须先写好 `dek_version` / `slots` / `manifest` / `recoveryBackup`**；本函数只负责签名与落盘。
2. 落盘成功后由编排层（§4.5）更新 anchor 的 `revision`。
3. **不做"内容相同就跳过"的优化**（v1 `export_vault_meta` 有该逻辑，`:244-247`）。v2 每次写都推进 `revision`，跳过会让 anchor 的单调性失真。

### 4.5 `SecurityDbState` 改动

文件：`src-tauri/crates/assistant-vault/src/security_db/mod.rs`。

| 项 | v1 现状 | v1.5 |
|---|---|---|
| `BOOTSTRAP_SALT`（`:21-24`） | 编译期常量 | **删除**，无保留用途（设计 §6.1） |
| `SQLCIPHER_PRAGMAS`（`:26-27`） | `kdf_iter = 256000` | `kdf_iter = 1`（P0-1） |
| `master_key: Mutex<Option<MasterKey>>` | 口令派生 | `dek: Mutex<Option<Dek>>` |
| `bootstrap_key: Mutex<Option<MasterKey>>` | 引导密钥 | `db_key: Mutex<Option<DbKey>>` |
| `get_master_key()`（`:238-246`） | `MutexGuard<'_, Option<MasterKey>>`，28 处 / 12 文件 | `get_dek() -> Result<MutexGuard<'_, Option<Dek>>, VaultError>`（**保持 guard 形态**），同步替换调用点 |
| `get_bootstrap_key()`（`:340-348`） | `MutexGuard<'_, Option<MasterKey>>`，7 处 / 3 文件 | `get_db_key() -> Result<MutexGuard<'_, Option<DbKey>>, VaultError>`，同步替换调用点 |
| `with_conn_and_key`（`:350-354`） | `FnOnce(&Connection, &MasterKey)` | `FnOnce(&Connection, &Dek)` |
| `init(&self, password)` | 两阶段 Argon2id | `init(&self, db_key: DbKey, dek: Dek, anchor: &Anchor) -> Result<(), VaultError>` |
| `unlock(&self, password)` | 两阶段 + 第三次 Argon2id（`mod.rs:116-196`，其中 `verify_master_password` 在 `:172-173`） | `unlock(&self, db_key: DbKey, dek: Dek, expected: &AnchorExpectation) -> Result<(), VaultError>` |

> **为什么按值传 `Dek` / `DbKey`**：`Dek` 等 newtype **不实现 `Clone`**（§2.1），`&Dek` 无法被复制进 `Mutex`。调用方（`lifecycle`）在本地构造后**移交所有权**；失败时 newtype 的 `Drop` 负责清零。这同时保证"同一把 `dek` 只存在于一个所有者手里"。

`AnchorExpectation` 定义在 `security_db/meta.rs`：

```rust
pub struct AnchorExpectation {
    pub meta_hash: String,
    pub dek_version: u32,
}
```

**`init` 行为**：

```
1. security.db 已存在 ──▶ AlreadyExists
2. 主密码强度检查（保留现有逻辑，`mod.rs:74-82`）
3. 建目录 → Connection::open → PRAGMA key = x'db_key' → SQLCIPHER_PRAGMAS
4. 执行 schema::INIT_SQL → meta::ensure_schema_version
5. meta::set_anchor(&conn, anchor)（revision = 0）
6. 写入 dek / db_key / conn
```

**`unlock` 行为**（对应设计 §2.6 第 5–6 步）：

```
1. PRAGMA key + SQLCIPHER_PRAGMAS
2. 探针读：SELECT value FROM vault_meta WHERE key = 'schema_version'
   └─ 失败（SQLCIPHER_NOT_DB / 无此表 / 空结果）──▶ LocalDbKeyMismatch
3. 读 anchor：
   ├─ anchor_meta_hash != expected.meta_hash ──▶ VaultRollbackDetected
   ├─ anchor_dek_version != expected.dek_version ──▶ VaultRollbackDetected
   └─ anchor 三键缺失 ──▶ 视为"库被重建"，按 §4.7 处理
4. 写入 dek / db_key / conn
```

约束：

1. **探针读必须存在**。v1 的 `unlock` 无验证读，只在 `PRAGMA key` 与 `SQLCIPHER_PRAGMAS` 之后直接继续（`mod.rs:134-135`），导致错误密钥直到首次业务查询才暴露。设计 §9.2 I12 把这条推广到生物识别路径，口令路径同样适用。
2. 探针读的 SQL **照抄恢复码路径**（`recovery_key/mod.rs:195-198`）——统一为 `SELECT value FROM vault_meta WHERE key = 'schema_version'`。
3. `unlock` 失败时**不得**把 `conn` / `dek` / `db_key` 写入状态（避免半解锁）。

### 4.6 anchor 读写与校验（补 G7）

文件：`src-tauri/crates/assistant-vault/src/security_db/meta.rs`。

```rust
pub fn get_anchor(conn: &Connection) -> Result<Option<Anchor>, String>;
pub fn set_anchor(conn: &Connection, anchor: &Anchor) -> Result<(), String>;
```

**`metaHash` 算法**：`SHA-256` over **`.vault-meta` 文件的完整字节**（含 `integrity` 字段，含缩进与换行），输出**小写 hex**。

理由：

- 覆盖整文件 → 任何字段改动（包括 `integrity` 自身）都会改变 hash；
- 不依赖 §3.7 的规范化规则 → anchor 与 integrity 是两条独立检测链，一条失效另一条仍有效（设计 §2.8 L1 vs L3 的分层意图）；
- 缩进敏感是**有意的**：v2 用 `to_string_pretty` 固定格式，任何外部工具重排 JSON 都会触发 `VaultRollbackDetected`，属于"应当被发现的异常"。

**`revision` 递增规则**：

| 时机 | 动作 |
|---|---|
| `initialize` | `revision = 0` |
| 每次 `write_vault_meta_v2` 成功后 | `revision += 1` |
| `LocalDbRebuilt`（§4.7 选项 2） | `revision` 重置为 **0**（基线重建，设计 §2.8 明确记录此失效点） |
| `unlock` | **只读不写**（否则每次解锁都改库，破坏"只读解锁"语义） |

**校验顺序**（`unlock` 内，见 §4.5）：先探针读（证明 `dbKey` 对），再比 anchor（证明 meta 未被回滚）。顺序不可交换——`dbKey` 错时 anchor 可能属于另一把钥匙的库，比对无意义。

### 4.7 `LocalDbKeyMismatch` 修复入口

文件：`src-tauri/crates/assistant-vault/src/security_db/rekey.rs`。

```rust
pub fn repair_local_db_with_old_password(
    &self,
    old_password: &str,
    new_password: &str,
    repo: &VaultFileManager,
    anchor: &Anchor,
) -> Result<(), VaultError>;

pub fn rebuild_local_db(
    &self,
    repo: &VaultFileManager,
    dek: &Dek,
    db_key: &DbKey,
    anchor: &Anchor,
) -> Result<(), VaultError>;
```

**`repair_local_db_with_old_password`**（设计 §3.5 选项 1）：

```
1. 从 .vault-meta 取当前 kdf/salt → 用 old_password 派生 KEK_old
2. dbKey_old = HKDF(KEK_old, "i9:v1:sqlcipher:" + vaultId)
3. 打开 security.db（PRAGMA key = x'dbKey_old'）→ 探针读确认
   └─ 失败 ──▶ LocalDbKeyMismatch（旧口令也不对，转选项 2）
4. PRAGMA rekey = x'dbKey_new'（dbKey_new 由新口令派生）
5. set_anchor(anchor)，revision 保留当前值 +1
6. 审计 LocalDbRepaired
```

**`rebuild_local_db`**（设计 §3.5 选项 2）：

```
1. 关闭现有 conn，删除 security.db（保留 .bak 一份）
2. 用 db_key 新建库 + INIT_SQL
3. 从 entries/*.enc 重建 vault_entries 索引（复用现有 import 路径）
4. 从 .audit/*.jsonl 回放审计（§8.4）
5. set_anchor(anchor)，revision = 0
6. 审计 LocalDbRebuilt
```

约束：

1. **两条路径都必须可用**。选项 2 是"忘了旧口令"时的唯一出路（设计 §3.5 "保证用户永远不会彻底卡死"）。
2. 重建的代价是 `anchor` 基线重置，**必须在日志与 UI 里写明**（设计 §2.8、附录 C G5）。
3. `security.db` 是缓存，**删除前不要求用户导出**（条目真身在 `entries/*.enc`）。

### 4.8 `change_master_password` 重写（O(1)）

文件：`rekey.rs`，签名改为：

```rust
pub fn change_master_password(
    &self,
    repo: &VaultFileManager,
    current: &str,
    new: &str,
) -> Result<VaultMetaFileV2, VaultError>;
```

流程（对应设计 §3.2，**顺序不可调换**）：

```
1. read_vault_meta_v2 → slot
2. KEK_old = derive_kek(current, slot.kdf) → dek = open_dek(KEK_old, wrappedDek, AAD)
   └─ GCM 失败 ──▶ InvalidPassword
3. 新 salt(16) → KEK_new = derive_kek(new, salt_new, 同参数)
4. dbKey_new = derive_db_key(KEK_new, vaultId)
5. 备份 security.db → PRAGMA rekey = x'dbKey_new'      ← 本机库先换钥匙
6. slot' = { id: pwd-{n+1}, kdf: 新 salt, wrappedDek: seal_dek(KEK_new, dek, AAD(slot', dekVersion), createdAt: now }
7. write_vault_meta_v2（原子写）                        ← 提交点
8. anchor = { meta_hash: 新文件 hash, dek_version 不变, revision += 1 }
9. repackage_biometric_payload（§5.5）
   ├─ 成功 ──▶ 继续
   └─ 失败 ──▶ 删除 vault-fingerprint（标记待重注册）+ warn，**不阻断**（设计 §3.6）
10. 返回新 meta
```

**删除的 v1 逻辑**：`:70-207` 的全量条目重加密段（`BEGIN TRANSACTION` 于 `:70`，`re_encrypt_result` 闭包 `:73-167`，`set_salt` 于 `:174-175`，`COMMIT` 于 `:199`）、`:208` 之后的旧 `PRAGMA rekey`、`meta::get_salt("hash_salt")`（`:34-35`）、`verify_master_password`（`:38-41`）、`backup_conn`（`:43`）。

**为什么第 9 步在第 7 步之后**：重包需要新的 `dbKey`，而 `dbKey` 在 meta 提交后才对外可见；同时重包失败不能回滚已提交的改密（设计 §3.6 "改密成功优先"）。第 9 步失败时用户下次生物识别解锁会得到 `BiometricReset`，而口令路径完全正常——这是可接受的降级。

**崩溃窗口**（第 5 步与第 7 步之间）：`security.db` 已是新钥匙、`.vault-meta` 仍是旧 slot。表现为"旧口令能解出 `dek`（meta 旧），但打不开库"。检测路径：`unlock` 第 2 步探针读失败 → `LocalDbKeyMismatch` → §4.7 选项 1（输入旧口令重包）或选项 2（重建）。**不需要任何额外文件**（设计 §3.2 要点）。

### 4.9 `rotate_dek`

`lifecycle::rotate_dek`（§4.1）流程：

```
1. 删除生物识别凭据（§5.6）：delete_key("vault-payload") 失败 ──▶ 立即返回错误，仓库未动
2. dek_new = Dek::generate()
3. 全量重加密：逐条 read .enc → open(dek_old, entryKey_old) → seal(dek_new, entryKey_new) → 原子替换
4. 库内加密列同样重加密（crypto_ops / entries 路径）
5. slot' = seal_dek(当前 KEK, dek_new, AAD(slot', dekVersion+1))
6. dek_version += 1；manifest 全量重算；recovery_backup = None
7. write_vault_meta_v2；anchor 更新（revision += 1）
8. 返回 (新 meta, encode_paper(&dek_new))
```

约束：

1. `dbKey` **不变** → **不调用 `PRAGMA rekey`**（设计 §3.3、§10 D7）。
2. `vaultId` **不变**（设计 §10 D10）。
3. 第 3–4 步失败必须**整体回滚**：先把全部条目写入临时目录，全部成功后再批量 rename；任一失败则删除临时目录并返回错误。**禁止边写边替换**（会产生新旧混合的仓库，manifest 无法表达）。
4. `recovery_backup` 必须清空——旧纸面已失效（设计 §3.3 第 5 步）。
5. 生物识别凭据删除放在**第 1 步**（任何破坏性写入之前）。理由：R8 的安全前提是"凭据已删除"，删除失败必须能干净中止；若放在末尾，此时条目已按 `dek_new` 重加密，中止将留下"新密文 + 旧 meta"的不一致状态。代价是轮换中途失败时用户需重新注册生物识别——无安全损失，只是多一步操作。
6. **禁止在持有 `get_dek()` guard 时调用 `state` 的写方法**（`init`/`unlock`/`set_dek`）——`std::sync::Mutex` 非重入，会自锁死。正确顺序：取 guard → 完成第 3–6 步 → **显式 `drop(guard)`** → 第 7 步之后写入新 `dek`。
7. 轮换成功**不自动重注册**生物识别；由 UI 在成功后引导用户重新注册（§5.6 第 4 步）。

### 4.10 `derive_entry_key` 替换点

设计 §6.1 列出 **17 处 / 7 个文件**：

| 文件 | 处数 |
|---|---|
| `security_db/crypto_ops.rs` | 6 |
| `security_db/rekey.rs` | 3（本次全部随 `change_master_password` 重写而消失） |
| `security_db/entries/write.rs` | 2 |
| `security_db/entries/import.rs` | 2 |
| `vault_file/mod.rs` | 2 |
| `security_db/entries/snapshot.rs` | 1 |
| `security_db/entries/deleted.rs` | 1 |

**机械替换规则**：`derive_entry_key(mk, id)` → `derive_entry_key(dek, id)`，参数类型从 `&MasterKey` 变 `&Dek`，函数体内无其他改动。替换后必须全仓搜索 `derive_entry_key(` 确认无 `MasterKey` 实参残留。

---

## 5. Step 5 — 生物识别层

涉及文件：

| 文件 | 改动性质 |
|---|---|
| `src-tauri/crates/assistant-vault/src/security_db/biometric_ops.rs` | 主改：payload、指纹、探针、审计 |
| `src-tauri/crates/assistant-vault/src/biometric/mod.rs` | trait 增加 ACL 判定方法（§5.7） |
| `src-tauri/crates/assistant-vault/src/biometric/{windows,macos,linux}.rs` | ACL 判定 + `delete_key` 区分 not-found / 真实失败（§5.8 A-1） |
| `src-tauri/src/cmd/vault/biometric.rs` | **签名不变**，只跟随类型改名 |

### 5.0 现状盘点（先确认要动哪些符号）

| 现有符号 | 位置 | v1.5 处理 |
|---|---|---|
| `KEY_ID_MASTER` / `KEY_ID_AES_KEY` / `KEY_ID_CIPHERTEXT` / `KEY_ID_SIG_HASH` | `biometric_ops.rs:14-17` | **改名**（§5.1）；**不保留旧名、不做 legacy 兼容**（无存量用户，设计 §5） |
| `derive_wrapping_key_from_signature` | `:22-28` | **保留原样**，参数冻结见 §5.1.5（salt / info 不改） |
| `compute_signature_hash` | `:30-35` | **保留原样**，哈希对象冻结见 §5.1.5（原始签名字节） |
| `restore_keys_from_plaintext` | `:37-88` | **重写**（§5.3） |
| `enroll_biometric_with_hint` | `:90-141` | **改内部**，签名不变（§5.4） |
| `unlock_biometric` | `:143-163` | **保留**（平台分派不变） |
| `disenroll_biometric` | `:166-180` | **改**：只删新 id（§5.8） |
| `is_biometric_enrolled` | `:183-197` | **改**：只查新 id（漏改 = 永远显示"未开启"） |
| `check_biometric_consistency` | `:199-221` | **改**：查新 id + 清理半注册残留（§5.3 约束 4） |
| `enroll_windows` / `unlock_windows` | `:226-290` | **改内部**：payload 与 key id 改；`vault-sig-hash` 校验、解密失败映射按 §5.3.1 |
| `enroll_separated` / `unlock_separated` | `:292-343` | **改内部**：payload 与 key id 改；条目缺失与解密失败映射按 §5.3.1 |
| `VaultCrypto::derive_sqlcipher_key(&bootstrap_key)` | `:55` | **删除**（§5.3 约束 3） |

### 5.1 key id 与 payload 布局

payload 长度：**52B** = `dek(20) ‖ dbKey(32)`。v1 为 64B `BK‖MK`（校验在 `:42-45`）。

新增常量（`biometric_ops.rs` 顶部）：

```rust
const BIOMETRIC_PAYLOAD_LEN: usize = 52;
const BIOMETRIC_DEK_LEN: usize = 20;
```

key id **分平台**定义（不是统一 5 个）：

| 平台 | key id | 存什么 | 保护方式 |
|---|---|---|---|
| 共用 | `vault-payload` | 包裹后的 52B payload 密文 | OS keystore |
| 共用 | `vault-fingerprint` | `hex(SHA-256(payload))` **明文** | OS keystore |
| Windows | `vault-sig-hash` | `hex(SHA-256(signature))` | OS keystore |
| 非 Windows | `vault-wrapping-key` | `hex(随机 32B AES key)` | 平台 ACL（§5.7） |

旧 id（**仅作事实记录，代码中不出现**）：

```
vault-master-key / vault-master-aes-key / vault-master-ciphertext / vault-master-sig-hash
```

> **改名是必须的，不是洁癖**：v1 的 `vault-master-key` 里存的是 64B `BK‖MK`。沿用旧名会让"升级后首次生物识别解锁"读到 64B 旧 payload，被 §5.3 第 1 步判为长度错误 → 用户看到"请重新注册"但重新注册写的是新名、读的是旧名，**永久死循环**。改名即可，旧条目成为孤儿。
>
> **不写 legacy 清理代码**。设计 §5 已确认"未投入生产、无存量用户"，为不存在的用户保留 4 个字符串常量与一条删除路径，正是 §5 点名的"把 v1 死代码与攻击面留在仓库里"。会留下旧条目的只有**开发机**（本地跑过 v1 代码的机器），处理方式是手动清理：Windows 在「设置 → 账户 → 登录选项」中删除 `assistant-vault-biometric:vault-master-*` 凭据，macOS / Linux 在系统钥匙串中删除同名条目。此说明只写在本规格，不进生产代码。
>
> 例外见 §5.8：`vault_reset_repository` 与 `rotate_dek` 产生的孤儿是**新流程自产**的，必须清理。

### 5.1.5 签名与派生参数（冻结）

Windows 路径的挑战值、HKDF 参数、`vault-sig-hash` 的哈希对象都是**常量**。v1.5 **全部沿用 v1 取值，一个字节都不改**：

| 项 | 值 | 来源 |
|---|---|---|
| 挑战（challenge） | `SHA-256("assistant-vault-biometric-v1")` → 32B | `biometric/windows.rs` `biometric_challenge()`（`:101-105`） |
| 签名算法 | `KeyCredentialManager` 平台默认（TPM 或软件密钥对），不可配置 | Windows API |
| 签名调用 | `KeyCredential::RequestSignAsync(challenge)`，**每次调用都触发 Hello 验证** | `windows.rs` `request_signature`（`:227` 起） |
| HKDF 算法 | HKDF-SHA256 | `security_db/biometric_ops.rs` `derive_wrapping_key_from_signature`（`:22-28`） |
| HKDF salt | `b"assistant-vault-v1"`（`HKDF_SALT`） | 同上 |
| HKDF info | `b"aes-256-gcm-key"`（`HKDF_INFO`） | 同上 |
| HKDF 输出 | 32B，直接作 AES-256-GCM 密钥 | 同上 |
| `vault-sig-hash` | `lowercase_hex(SHA-256(signature))`，`signature` 为 `RequestSignAsync` 返回的**原始签名字节** | `compute_signature_hash`（`:30-35`） |

约束：

1. **挑战是公开且固定的常量，因此签名是确定性的**。`vault-sig-hash` 能够比对成功，本身就证明签名可复现——它必须是常量，否则设计 R8「删凭据才能吊销」的立论不成立（若签名每次不同，旧凭据根本无法解开新 payload，也就不需要 §5.6）。

2. **`vault-sig-hash` 的哈希对象是原始签名字节**，不是 base64 文本、不是 HKDF 输出。任一端写错，症状是「TPM credential has changed」误报或「解密失败」，与真正的凭据变更无法区分。

3. **HKDF salt 保持 `assistant-vault-v1` 不变（已裁决）**。它是**生物识别密钥派生域**的标签，不是 `.vault-meta` 的格式版本（后者为 v2），两者不可混淆。改名虽可在两端同步后自洽，但 v1.5 已用新 key id + 新 payload，**无任何迁移需求**，改名无收益、只增加「注册用 A、解锁用 B」的错配风险。本条与 §5.0「`derive_wrapping_key_from_signature` 保留原样」一致。

4. **非 Windows 路径没有签名，本节参数不适用**。其 wrapping key 是注册时 `rand` 生成的 32B 随机值（`enroll_separated`），以 hex 存 `vault-wrapping-key`；payload 以 AES-256-GCM 加密后存 `vault-payload`。因此非 Windows **没有**「凭据已变更」信号，只能靠 ACL 门控（§5.7）。

5. **挑战字符串不进入 payload 加密的 AAD**。它只决定签名值；payload 完整性由 AES-256-GCM 认证标签承担。

6. **签名与 wrapping key 必须 zeroize**（`enroll_windows` / `unlock_windows` 现有代码已做，重写时不得丢失）。

### 5.2 fingerprint

```rust
fn compute_fingerprint(payload: &[u8]) -> String;   // 小写 hex(SHA-256(payload))
```

- 与 payload **一起**存 keystore（key id `vault-fingerprint`），设计 §3.6。
- **明文 hex，不加密**——只用于"是否过期"的比对，SHA-256 单向，不含密钥信息。
- **必须对明文 52B payload 计算，不能对密文计算**：非 Windows 路径每次注册都会生成新的随机 wrapping key（`enroll_separated`），密文随之变化；若指纹基于密文，则"重包后内容其实没变"也会被判为过期，导致每次改密都误报 `BiometricReset`。

### 5.3 `restore_keys_from_plaintext` 重写

```rust
fn restore_keys_from_plaintext(
    state: &SecurityDbState,
    payload: &VaultPlaintext,
    key_store: &dyn BiometricKeyStore,
) -> Result<(), VaultError>;
```

行为（补 G12，对应设计 §9.2 I12 / R10）：

> **前置**：本函数只处理**已解密的明文 payload**。签名校验与 payload 解密由平台入口完成（§5.3.1），失败在那里就地返回 `BiometricReset`，不会进入本流程。

```
1. payload.len() != 52
   ──▶ 审计 BiometricFailed{reason:"payload_len"}（JSONL）→ BiometricReset
2. dek = payload[0..20]，db_key = payload[20..52]
3. 读 keystore 的 vault-fingerprint，与 compute_fingerprint(payload) 比对
   ├─ 条目缺失 ──▶ 审计 BiometricFailed{reason:"stale_payload"}（JSONL）→ BiometricReset
   └─ 值不符   ──▶ 审计 BiometricFailed{reason:"stale_payload"}（JSONL）→ BiometricReset
4. 用 db_key 打开 security.db（PRAGMA key + SQLCIPHER_PRAGMAS）
5. 探针读：SELECT value FROM vault_meta WHERE key = 'schema_version'
   └─ 失败 ──▶ 审计 BiometricFailed{reason:"probe_failed"}（JSONL）→ BiometricReset
6. 读 anchor，比对 meta_hash / dek_version
   └─ 不符 ──▶ 审计 AnchorRollbackRejected（conn + JSONL）→ VaultRollbackDetected
7. 写入 dek / db_key / conn
8. 审计 BiometricUnlocked（conn + JSONL）
```

约束：

1. **第 5 步探针读必须存在**。v1 只做 `PRAGMA key` + `SQLCIPHER_PRAGMAS` 就写入 `conn`（`:60-63`），产生"假解锁"（设计 I12）。探针失败必须返回 `BiometricReset`，**不得**进入已解锁状态。

2. **指纹检查（第 3 步）必须早于开库（第 4 步）**。若 payload 已过期（`dbKey` 是旧的），用旧 `dbKey` 开库必然让第 5 步探针失败，用户会看到"数据库损坏"而非"请重新注册"。先查指纹能给出正确诊断，也省一次 SQLCipher 初始化。

3. **禁止二次派生 SQLCipher 密钥**。v1 在 `:55` 调 `VaultCrypto::derive_sqlcipher_key(&bootstrap_key)`；v1.5 的 `dbKey` **本身就是页密钥**，直接 `PRAGMA key`。若保留该调用，口令路径用 `dbKey` 开库、生物识别路径用 `HKDF(dbKey)` 开库，**两条路径永远互相打不开**，且失败点在探针读 → 被误判成 `BiometricReset`，用户反复重新注册也无解。必须删除，并在 §3 一并删除 `derive_sqlcipher_key` 的所有 4 个生产调用点（`security_db/mod.rs:77`、`mod.rs:130`、`biometric_ops.rs:55`、`recovery_key/mod.rs:173`）。

4. **`check_biometric_consistency` 的归属**。v1 在 `:76-88` 于开库前调用它。v1.5 保留，但**位置提前到第 3 步之前**，语义改为：

   | 平台 | 判定 | 处理 |
   |---|---|---|
   | Windows | `has_key("vault-payload")` 为 false | 若 `has_key("vault-fingerprint")` 为 true，删之（清半注册残留）→ `BiometricReset` |
   | 非 Windows | `vault-wrapping-key` 或 `vault-payload` 任一缺失 | 删除三者（含 fingerprint）→ `BiometricReset` |

   它承担"半注册残留清理"职责，不能因为新增指纹检查就删掉——指纹条目存在但 payload 缺失时，第 3 步会走到"读不到 payload"而无法区分原因。

5. **任何失败都不得写入 `conn` / `dek` / `db_key`**。第 3–6 步的 `conn` 只能是局部变量，成功后才 `mem::swap` 进 `state`。

6. **第 3/5 步的失败审计只能写 JSONL**。这两处 `conn` 不可用（未打开或探针已失败），必须走 `audit::log_dual(None, Some(writer), ..)`（§8.2）。第 6 步起 `conn` 可用，走双写。JSONL 写入失败只 `log::warn!`，**不阻断**生物识别解锁（避免审计目录不可写造成永久锁死）。

7. **指纹不是防回滚机制**。攻击者若能同时替换 payload 与 fingerprint，指纹依然匹配。防回滚由第 6 步 anchor 承担（§3.7）。文档与代码注释都必须写明这一点，避免后续误以为指纹有安全作用。

### 5.3.1 平台解锁入口（`unlock_windows` / `unlock_separated`）

`restore_keys_from_plaintext` 收到的参数 `payload: &VaultPlaintext` 是**已解密的明文 payload**——解密发生在平台入口函数里（v1 即如此，`unlock_windows:279`、`unlock_separated:335`）。因此**签名校验与解密失败的处理写在这两个函数中，不在 8 步流程内**。

**Windows（`unlock_windows`）**：

```
1. blob = get_key("vault-payload")
   └─ 条目缺失 ──▶ BiometricReset
2. signature = request_signature("vault-payload")        // 此处弹 Hello
   └─ BiometricUnavailable ──▶ 原样返回，不审计 BiometricFailed
3. sig_hash = compute_signature_hash(signature)          // §5.1.5
4. stored = get_key("vault-sig-hash")
   ├─ 条目缺失           ──▶ 审计 BiometricFailed{reason:"sig_hash_missing"}（JSONL）→ BiometricReset
   └─ 值与 sig_hash 不符 ──▶ 审计 BiometricFailed{reason:"sig_changed"}（JSONL）→ BiometricReset
5. wrapping_key = HKDF-SHA256(signature, HKDF_SALT, HKDF_INFO)   // §5.1.5
6. plaintext = decrypt_field(blob, wrapping_key)
   └─ 失败 ──▶ 审计 BiometricFailed{reason:"decrypt_failed"}（JSONL）→ BiometricReset
7. zeroize(signature) / zeroize(wrapping_key)
8. restore_keys_from_plaintext(state, &plaintext, key_store)   // §5.3 的 8 步
```

**非 Windows（`unlock_separated`）**：

```
1. key_hex = get_key("vault-wrapping-key")
   └─ 条目缺失 ──▶ BiometricReset
2. blob = get_key("vault-payload")
   └─ 条目缺失 ──▶ BiometricReset
3. wrapping_key = hex_decode(key_hex)，长度必须为 32
   └─ 失败 ──▶ 审计 BiometricFailed{reason:"wrapping_key_len"}（JSONL）→ BiometricReset
4. plaintext = decrypt_field(blob, wrapping_key)
   └─ 失败 ──▶ 审计 BiometricFailed{reason:"decrypt_failed"}（JSONL）→ BiometricReset
5. zeroize(wrapping_key)
6. restore_keys_from_plaintext(state, &plaintext, key_store)   // §5.3 的 8 步
```

约束：

1. **Windows 的 `vault-sig-hash` 校验（第 4 步）必须早于解密（第 6 步）**。凭据被重建（用户重置 Hello、更换设备密钥）后，payload 密文未变、§5.2 指纹也仍匹配，但新签名派生的 wrapping key 解不开旧密文。若先解密，用户看到的是「解密失败」；先比对 sig-hash 才能给出「凭据已变更，请重新注册」——这正是设计 R10 要消除的错误诊断。

2. **解密失败必须映射为 `BiometricReset`，不得保留 `VaultError::Crypto`**。v1 在 `:279-280`、`:335-337` 用 `map_err(VaultError::Crypto)`，前端无法据此给出「重新注册」引导，用户只会反复重试。

3. **`request_signature` 返回 `BiometricUnavailable` 时不得审计 `BiometricFailed`**。这是用户主动取消或凭据不存在，不是异常。只有 Hello 通过但后续校验失败才记 `BiometricFailed`。

4. **这两个函数在失败时不得写入 `state` 的 `conn` / `dek` / `db_key`**（与 §5.3 约束 5 同一要求）。v1 的 `unlock_separated` 直接把解密结果交给 `restore_keys_from_plaintext`，而后者先写状态后校验（`:66-88`），是"假解锁"的来源之一。

5. **`BiometricReset` 的语义是「payload 已不可用，需重新注册」**，上层**不得**把它当作「解锁失败」自动重试——重试不会成功，只会重复弹 Hello。UI 必须直接切换到「重新注册」引导。

### 5.4 注册

**签名必须保留回调参数，不得改成 `hint: Option<&str>`**：

```rust
pub fn enroll_biometric_with_hint<F>(
    &self,
    key_store: &dyn BiometricKeyStore,
    on_credential_ensured: F,
) -> Result<(), VaultError>
where
    F: FnOnce();
```

理由：该回调在 `src-tauri/src/cmd/vault/biometric.rs:73-80` 用于广播 `BIOMETRIC_ENROLL_STEP = "credential-created"`，让 UI 在"系统凭据已创建、等待指纹验证"时切换文案。Windows 上首次注册需用户抬手验证，无此事件会卡在无反馈的等待态。

行为：

```
1. 平台 ACL 判定（§5.7）──▶ 不满足则 BiometricUnavailable（不写任何条目）
2. dek = state.get_dek()?，db_key = state.get_db_key()?   // 任一缺失 → VaultError::Locked
3. payload = dek ‖ db_key（52B）
4. fingerprint = compute_fingerprint(payload)
5. key_store.set_key("vault-payload", 包裹后的 payload)
   └─ Windows: 内部先 ensure_credential，成功后调 on_credential_ensured()；
              加密密钥 = HKDF-SHA256(sig, HKDF_SALT, HKDF_INFO)（§5.1.5）
   └─ 非 Windows: 生成随机 32B wrapping key → set_key("vault-wrapping-key") → 加密 payload
6. key_store.set_key("vault-fingerprint", fingerprint.as_bytes())
   └─ Windows 额外 set_key("vault-sig-hash", compute_signature_hash(sig))
7. 审计 BiometricEnrolled（conn + JSONL）
```

约束：

1. **先写 payload 再写 fingerprint**。反序会在中途失败时留下"指纹在、payload 无"的残留，被 §5.3 约束 4 判为 `BiometricReset`——虽然能自愈，但会多一次无谓的注册失败提示。
2. 注册是**幂等**的：重复注册直接覆盖，不要求先反注册。
3. 非 Windows 路径每次注册都生成**新的**随机 wrapping key，`vault-wrapping-key` 必须与 `vault-payload` 同时更新；先删旧的再写新的（`set_key` 内部已做覆盖，但 macOS 的实现是先 search+delete 再 add，需保证两步都成功）。
4. **不做旧 key id 清理**（§5.1）。注册路径只碰 4 个新 id。
5. **注册与解锁必须使用同一套挑战与 HKDF 参数**（§5.1.5）。任一端改动都会导致"注册用 A、解锁用 B"，症状为 `decrypt_failed`，与凭据变更无法区分。

### 5.5 自动重包与失败标记

```rust
pub fn repackage_biometric_payload(&self, key_store: &dyn BiometricKeyStore)
    -> Result<(), VaultError>;
```

调用时机：

| 场景 | 调用者 | 失败处理 |
|---|---|---|
| 改密（`dbKey` 变） | `lifecycle::change_password` **第 9 步**（§4.8） | **不阻断**改密；标记待重注册（设计 §3.6） |
| `rotate_dek`（`dek` 变） | **不调用本函数**；改走 §5.6 的"删除凭据" | R8 要求删除凭据而非重包，删除失败即中止 |

"标记待重注册"的实现：**删除 fingerprint 条目**（key id `vault-fingerprint`）。下次 `restore_keys_from_plaintext` 第 3 步读不到 fingerprint → 返回 `BiometricReset`，提示重新注册。**不引入额外的标志位**——少一个可被篡改的状态。

注意：此机制**仅用于改密路径**（`dbKey` 变，payload 内容过期但仍存在）。`rotate_dek` 不走这条，而是直接删凭据（§5.6）。

### 5.6 R8：`rotate_dek` 强制重注册

```
1. key_store.delete_key("vault-payload")
   ├─ 成功 ──▶ 继续
   └─ 失败 ──▶ 审计 BiometricFailed{reason:"delete_failed"}（JSONL）
              ──▶ 返回错误，rotate_dek 不得宣告成功
2. 依次删除 vault-fingerprint / vault-sig-hash / vault-wrapping-key（失败只 warn）
3. 审计 BiometricRevoked
4. UI 提示"轮换数据密钥后需重新注册生物识别"
5. 用户拒绝重注册 ──▶ 视为"已停用生物识别"，显式告知（设计 R8）
```

理由（设计 R8）：wrapping key 由固定公开挑战的签名派生，是常量；`rotate_dek` 只换 payload 里的 `dek`，旧签名仍能解出新 payload。**不删凭据 = 旧凭据仍然有效**。

约束：

1. **第 1 步失败必须中止**。这是全流程唯一"删除失败等于安全前提落空"的位置——R8 的立论完全建立在"凭据已被删除"上。只 `warn` 放过会让旧凭据继续可用，与设计结论直接矛盾。
2. **`rotate_dek` 的执行顺序**：删除凭据必须是 §4.9 的**第 1 步**（任何破坏性写入之前）。若放在末尾，条目已按 `dek_new` 重加密，此时删除失败而中止，会留下"新密文 + 旧 meta"的不一致仓库。
3. 第 2 步的 3 个条目失败只 `warn`：`vault-payload` 已删时，剩余条目无法构成可用的解锁路径（缺 payload 即 `check_biometric_consistency` 判 `BiometricReset`），属于可自愈残留。
4. **不得**把失败降级为"标记待重注册"（§5.5 的机制）——那只适用于"payload 已过期但仍存在"的情形，与"凭据仍可解锁"是两回事。

### 5.7 R9：`set_key_unprotected` 回落

`biometric/mod.rs:24-26` 的非 Windows 路径把 wrapping key 与密文存入同一 key store。设计 R9 要求逐平台确认 ACL：

| 平台 | 要求 | 不满足时 |
|---|---|---|
| macOS | `kSecAccessControlUserPresence` | **禁用生物识别** |
| Linux | secret service 绑定会话锁 | **禁用生物识别** |
| Windows | Hello（TPM） | 已满足 |

**禁止静默降级**：`set_key_unprotected` 回落路径必须在编译期或运行时显式判定；判定不通过时 `enroll_biometric_with_hint` 返回 `BiometricUnavailable`，而不是"存了但任何人都能解"。

### 5.8 重置与轮换的 keystore 清理

前面 §5.1 已说明"不为 v1 存量条目写兼容代码"。但 `vault_reset_repository` 与 `rotate_dek` 产生的孤儿是**新流程自产**的，必须清理，否则会造成用户可见的错误状态。

**A. `vault_reset_repository` 必须清空 keystore**

设计 §5 第 3 步只写"删除 `.vault-meta` / `security.db` / `entries/`"，漏了 OS keystore。若不清，重置后会出现如下链条：

```
旧 vault-payload 仍在
  └─ is_biometric_enrolled() 查 has_key("vault-payload") → true
     └─ UI 显示"已开启生物识别"（而新库从未注册过）
        └─ 用户点生物识别解锁
           └─ 指纹比对：旧 payload 与旧 fingerprint 自身一致 → 通过
              └─ 用旧 dbKey 开新库 → 探针读失败
                 └─ BiometricReset → "请重新注册"
```

用户看到的现象是**刚重置完的干净库提示"已开启生物识别"，一解锁却说"请重新注册"**。Windows 上旧凭据还会永久留在 Hello 凭据列表。

要求：

```rust
fn clear_all_biometric_entries(key_store: &dyn BiometricKeyStore) -> Result<(), VaultError>;
```

- 删除全部 4 个新 id：`vault-payload` / `vault-fingerprint` / `vault-sig-hash` / `vault-wrapping-key`。
- 调用时机：`vault_reset_repository` 删除文件**之前**（§9.3 第 4 步），先清 keystore 再删文件——若顺序反了且 keystore 清理失败，库已删但凭据仍在，用户失去任何可参照的上下文。
- 清理失败**必须中止重置并报错**：重置是破坏性操作，失败后保留库文件比"删了一半"更好。
- 审计 `BiometricRevoked`（`conn = None`，走 JSONL）。

**A-1. 前置改造：`delete_key` 必须能区分"不存在"与"删除失败"**

当前三个平台的实现都**吞掉**了失败（`has_key` 返回 false 却无法解释原因）：

| 平台 | 现状 | 必须改为 |
|---|---|---|
| macOS（`macos.rs:135-145`） | **已正确**：`is_item_not_found(e)` → `Ok(())`，其余 → `Err` | 不变 |
| Windows（`windows.rs:150-160`） | `if let Ok(op) = delete_op { let _ = wait_for_action(op); }` + `let _ = entry.delete_credential()`，**全部错误被丢弃**，恒 `Ok(())` | not-found → `Ok(())`；其余 → `Err(BiometricFailed)` |
| Linux（`linux.rs:225-248`） | `let _ = secret_password_clear_sync(..)`，`error` 只 `g_free` 后 `info!`，**恒 `Ok(())`** | `error.is_null()` 或 GError 为 not-found → `Ok(())`；其余 → `Err(BiometricFailed)` |

理由：§5.6 与本节都要求"删除失败即中止"，若 `delete_key` 恒返回 `Ok`，该约束在任何平台都无法生效——这会让 §5.6 的 R8 与 T-B1/T-B3 全部变成永远通过的假测试。

Windows 还需注意 `keyring_entry(key_id)?` 在 `DeleteAsync` 之后：该调用在凭据不存在时也可能失败，需一并归入 not-found 分支。

**B. `disenroll_biometric` 只删新 id**

`disenroll_biometric`（`:166-180`）改为删除 4 个新 id，**不含**旧 id。v1 的 4 个旧条目如仍存在，属开发机环境残留（§5.1），不在产品逻辑内。

**C. 与 §5.6 的分工**

| 场景 | 删除范围 | 失败处理 |
|---|---|---|
| `rotate_dek`（§5.6） | `vault-payload` 优先，其余 3 个尽力 | `vault-payload` 失败 → **中止**；其余失败 → warn |
| `vault_reset_repository`（本节 A） | 全部 4 个 | 任一失败 → **中止重置** |
| `disenroll_biometric`（本节 B） | 全部 4 个 | 失败 → 返回错误（用户显式操作，不能假装成功） |

---

## 6. Step 6 — 恢复通道层

### 6.1 删除清单

`recovery_key/mod.rs` 删除：

- `generate_recovery_key`（`:33`）
- `decrypt_recovery_keys`（`:79`）
- `unlock_with_recovery_key`（`:127`）
- `EmergencyData`（`:27-31`）
- `derive_recovery_key` / `parse_recovery_key` / `format_recovery_key`（v1 32 字符 4 组编码）
- `recovery_key/mod.rs:191` 的 `PRAGMA` 字面量（设计 §6.4 要求统一走 `SQLCIPHER_PRAGMAS`）
- `lib.rs` 中 `unlock_with_recovery_key` 的再导出（若有）

### 6.2 保留 + 新增

保留：§3.8 的 6 个纸面函数。
新增：`RecoverySession`（§2.5）+ `open_recovery_session`：

```rust
pub fn open_recovery_session(
    repo: &VaultFileManager,
    raw_key: &str,
) -> Result<RecoverySession, VaultError>;
```

流程（对应设计 §2.7，**顺序不可调换**）：

```
1. dek = decode_paper(raw_key)
   ├─ 格式错 ──▶ InvalidRecoveryKeyFormat
   └─ CRC 错 ──▶ InvalidRecoveryKeyChecksum
2. read_vault_meta_v2
   ├─ version=1 ──▶ UnsupportedVaultVersion
   └─ 其余非法 ──▶ VaultCorrupted
3. metaKey = derive_meta_key(&dek)
   verify_meta_integrity(metaKey)
   └─ 失败 ──▶ InvalidRecoveryKeyOrMetaTampered     ← 纸面抄错（CRC 侥幸通过）与 meta 篡改不可区分
4. verify_manifest()（逐条 sha256）
   └─ 失败 ──▶ VaultManifestMismatch
5. 构造 RecoverySession { dek, entries_dir, manifest }
```

约束：

1. **第 3 步的错误不可拆分**（设计 §2.7）：解码后的 `dek` 恒为 20 字节，长度不构成区分依据；CRC 已过而 HMAC 失败只可能是两种原因，统一返回一个码。
2. **完全不触碰 `security.db`**：不派生 `dbKey`、不调 `PRAGMA`、不读 anchor。这是"纸面开不了库"的实现级保证。
3. `open_recovery_session` **不写审计库**（没有连接）；导出结束后写 JSONL（§8.3）。

### 6.3 `RecoverySession::export_all`

```
1. 遍历 manifest.entries（按 id）
2. entryKey = derive_entry_key(&dek, id)
3. 读 entries/{id}.enc → decrypt_field(encrypted_blob, entryKey) → EntryBlob
4. 校验文件 sha256 与 manifest 一致（防止 3 与 1 之间被替换）
   └─ 不符 ──▶ VaultManifestMismatch
5. 返回 Vec<PlainEntry>
```

约束：

1. **不做任何写入**。连 `recoveryBackup` 标记都不写（那是导出纸面的动作，不是用纸面的动作）。
2. 单个条目解密失败 → 记入 `skipped`，**不中断整体**（与 v1 `export_all_entries` 的 `skipped_ids` 语义一致）。
3. 结束时调用方（命令层）写 JSONL `RecoveryExportUsed { entryCount, at }`（§8.3）。

### 6.4 `recoveryBackup` 写入时机（补 G18）

| 动作 | 写入者 | 内容 |
|---|---|---|
| `vault_export_recovery_key` 成功返回纸面 | 命令层 → `write_vault_meta_v2` | `recoveryBackup = { dek_version: 当前, confirmedAt: now }` |
| `rotate_dek` | `lifecycle::rotate_dek` | `recoveryBackup = None` |
| `initialize` | `lifecycle::initialize` | `recoveryBackup = None` |

"是否已备份"的查询：**不新增命令**。前端通过 `vault_get_status` 已有的返回里带 `recoveryBackup: Option<RecoveryBackup>` 字段（新增一个可选字段，见 §9.2）。

> 设计 §6.3 写"`vault_has_recovery_key` 删除（'是否已备份纸面'由 `recoveryBackup` 标记回答）"，但没有说前端怎么拿到该标记。本规格把它挂到状态查询上（附录 C-5）。

---

## 7. Step 7 — 错误码层（补 G2）

### 7.1 `VaultError` 新增变体

文件：`src-tauri/crates/assistant-crypto/src/error.rs`。

```rust
#[error("Unsupported vault version; reset required")]
UnsupportedVaultVersion,

#[error("Local vault database key mismatch")]
LocalDbKeyMismatch,

#[error("Vault rollback detected")]
VaultRollbackDetected,

#[error("Vault manifest mismatch")]
VaultManifestMismatch,

#[error("Vault meta is corrupted or unparsable")]
VaultCorrupted(String),

#[error("Invalid recovery key format")]
InvalidRecoveryKeyFormat,

#[error("Invalid recovery key checksum")]
InvalidRecoveryKeyChecksum,

#[error("Invalid recovery key or vault meta tampered")]
InvalidRecoveryKeyOrMetaTampered,
```

删除：

```rust
InvalidRecoveryKey,      // 无调用点后删除（原调用点全在 v1 恢复通道）
RecoveryKeyNotSet,       // 无调用点后删除
```

> `RecoveryKeyNotSet` 的唯一调用点是 `cmd/vault/recovery.rs:90`，随 `vault_disaster_recovery_export` 删除而消失。`InvalidRecoveryKey` 的调用点全在 `recovery_key/mod.rs`（`v1` 函数）与 `tests/recovery_key.rs`，同样消失。

### 7.2 `VaultErrorCode` 新增项

文件：`src-tauri/crates/assistant-vault/src/dto/error.rs`。

| 变体 | camelCase | retryable | 说明 |
|---|---|---|---|
| `UnsupportedVaultVersion` | `unsupportedVaultVersion` | 否 | 旧库，需重置（§4.3） |
| `LocalDbKeyMismatch` | `localDbKeyMismatch` | 否 | 本机库失配（§4.5 / §4.7） |
| `VaultRollbackDetected` | `vaultRollbackDetected` | 否 | anchor 不匹配（§4.6） |
| `VaultManifestMismatch` | `vaultManifestMismatch` | 否 | manifest 不符（§4.2 / §6.3） |
| `VaultCorrupted` | `vaultCorrupted` | 否 | 严格解析失败（§4.3） |
| `InvalidRecoveryKeyFormat` | `invalidRecoveryKeyFormat` | 否 | 纸面格式/长度/版本字节错（§3.8） |
| `InvalidRecoveryKeyChecksum` | `invalidRecoveryKeyChecksum` | 否 | CRC32C 不符（§3.8） |
| `InvalidRecoveryKeyOrMetaTampered` | `invalidRecoveryKeyOrMetaTampered` | 否 | 纸面抄错或 meta 篡改（§6.2） |

删除：`InvalidRecoveryKey`、`RecoveryKeyNotSet`。

`is_retryable` **保持不变**：当前实现只对 `RateLimited` 与 `GitUnavailable` 返回 true（`dto/error.rs:32-36`，`matches!` 在 `:34`）。新增的 8 项全部 **false**——它们都是"需要用户决策"的终态，重试无意义。

### 7.3 `convert.rs` 映射

文件：`src-tauri/crates/assistant-vault/src/dto/convert.rs`。

`VaultError → VaultErrorDto` 的 `match` 必须**穷尽**（不加 `_ =>` 兜底，让新增变体在编译期强制被处理）。新增 8 个 arm：

```rust
VaultError::UnsupportedVaultVersion => (VaultErrorCode::UnsupportedVaultVersion, e.to_string()),
VaultError::LocalDbKeyMismatch => (VaultErrorCode::LocalDbKeyMismatch, e.to_string()),
VaultError::VaultRollbackDetected => (VaultErrorCode::VaultRollbackDetected, e.to_string()),
VaultError::VaultManifestMismatch => (VaultErrorCode::VaultManifestMismatch, e.to_string()),
VaultError::VaultCorrupted(m) => (VaultErrorCode::VaultCorrupted, m.clone()),
VaultError::InvalidRecoveryKeyFormat => (VaultErrorCode::InvalidRecoveryKeyFormat, e.to_string()),
VaultError::InvalidRecoveryKeyChecksum => (VaultErrorCode::InvalidRecoveryKeyChecksum, e.to_string()),
VaultError::InvalidRecoveryKeyOrMetaTampered => (VaultErrorCode::InvalidRecoveryKeyOrMetaTampered, e.to_string()),
```

`VaultCorrupted` 携带 `String` 以保留解析细节；`InvalidRecoveryKeyOrMetaTampered` 为无载荷变体（§6.2 第 3 步的"不可区分"语义由码本身表达，无需附加信息）。

### 7.4 前端错误码数组

文件：`src/vault/core/vaultError.ts`。

`VAULT_ERROR_CODES` 数组（当前 **24** 项，手写维护）追加 8 项、删除 2 项：

```ts
'unsupportedVaultVersion',
'localDbKeyMismatch',
'vaultRollbackDetected',
'vaultManifestMismatch',
'vaultCorrupted',
'invalidRecoveryKeyFormat',
'invalidRecoveryKeyChecksum',
'invalidRecoveryKeyOrMetaTampered',
```

删除 `'invalidRecoveryKey'`、`'recoveryKeyNotSet'`。

**两处不同步的事实**：`VaultErrorCode` 的**类型**由 `ts_rs` 从 Rust 枚举导出（`dto/error.rs` 的 `#[ts(export, export_to = "vault.contract.ts")]`），落点是 `src/vault/core/contract/vault.contract.ts`；而 `src/vault/core/vault.types.ts` 是**手写的 barrel**（`export type { ... } from "./contract/vault.contract"`），**不会被 ts-rs 重写**。`VAULT_ERROR_CODES` **数组同样是手写的**（`vaultError.ts:5-30`）。因此流程是三步：改 Rust 枚举 → 重新生成 `contract/vault.contract.ts` → **手工同步 `VAULT_ERROR_CODES` 数组**。漏改数组的后果是 `isVaultErrorDto` 对新码返回 false，错误会被降级为 `String(value)`。

**`vault.types.ts` 的两处必改**（同属 §10 前端层）：

1. 头部注释第 2 行声明"唯一例外是 `.vault-meta` 文件内的 `exportedAt`（unix 秒）"——`exported_at` 在 §3 已被删除，该注释必须同步删除，否则前端会继续以为存在秒级字段。
2. 新增 DTO（`RecoveryKeyDto`、`RecoveryExportDto`、`PaperErrorPosDto`）由 ts-rs 生成到 `contract/vault.contract.ts` 后，**必须手工**在 barrel 中追加 `export type { ... }`，否则 `vault.service.ts` 无法导入。

**一致性测试**：由 `tests/dto_contract.rs` 断言（§11.2 T-D1）——Rust 侧 `serde_json` 序列化全部变体得到 camelCase 集合，与从 `src/vault/core/vaultError.ts` 文本中提取的字符串集合做**双向包含**比对，防止两端漂移。

---

## 8. Step 8 — 审计层

### 8.1 `AuditEvent` 扩展

文件：`src-tauri/crates/assistant-vault/src/audit/mod.rs`。

新增（`Display` 映射同时补齐）：

| 变体 | 字符串 |
|---|---|
| `RecoveryKeyExported` | `recovery_key_exported` |
| `RecoveryExportUsed` | `recovery_export_used` |
| `DekRotated` | `dek_rotated` |
| `LocalDbRepaired` | `local_db_repaired` |
| `LocalDbRebuilt` | `local_db_rebuilt` |
| `AnchorRollbackRejected` | `anchor_rollback_rejected` |
| `BiometricUnlocked` | `biometric_unlocked` |
| `BiometricFailed` | `biometric_failed` |
| `BiometricRevoked` | `biometric_revoked` |
| `VaultRepositoryReset` | `vault_repository_reset` |
| `VaultUnsupportedVersion` | `vault_unsupported_version` |

删除：`RecoveryKeyGenerated`（被 `RecoveryKeyExported` 取代）、`RecoveryKeyInvalidated`（改密不再使纸面失效——`dek` 未变）。

### 8.2 JSONL 双写（补 G15）

新增文件：`src-tauri/crates/assistant-vault/src/audit/jsonl.rs`。

```rust
pub struct AuditJsonlWriter { dir: PathBuf }

impl AuditJsonlWriter {
    pub fn new(dir: PathBuf) -> Self;
    pub fn append(&self, event: AuditEvent, detail: Option<&str>) -> Result<(), VaultError>;
    pub fn read_since(&self, since_ms: i64) -> Result<Vec<JsonlRecord>, VaultError>;
}

#[derive(Serialize, Deserialize)]
pub struct JsonlRecord { pub at: i64, pub event: String, pub detail: Option<String> }
```

| 项 | 规定 |
|---|---|
| 路径 | `<app_data_dir>/audit/` |
| 文件名 | `audit-YYYYMMDD.jsonl`（本地时区） |
| 行格式 | 单行紧凑 JSON：`{"at":<ms>,"event":"<display>","detail":<string\|null>}` |
| 写入方式 | `OpenOptions::new().create(true).append(true)` + 单次 `write_all` + 行尾 `\n` |
| 原子性 | 追加写，无 rename；崩溃最多丢最后一行 |
| 轮转 | 跨日新建文件；单文件 > 8 MiB 时新建 `audit-YYYYMMDD-{n}.jsonl` |
| 编码 | UTF-8 |

**双写封装**（`audit::log_dual`）：

```rust
pub fn log_dual(
    conn: Option<&Connection>,
    writer: Option<&AuditJsonlWriter>,
    event: AuditEvent,
    detail: Option<&str>,
) -> Result<(), VaultError>;
```

规则：

1. **两路都尝试，任一路成功即返回 Ok**；两路都失败才返回错误。审计不能因为某一路不可用就阻断业务（设计 §7.3）。
2. 连接与 writer 均为 `Option`：恢复通道（`conn = None`）、未初始化（`writer = None`）都能调。
3. `audit::log` 保留为 `log_dual(Some(conn), None, ..)` 的别名，**现有 40+ 调用点不必改签名**。

### 8.3 恢复通道的审计落点

| 事件 | 落点 | 理由 |
|---|---|---|
| `RecoveryKeyExported` | 命令层 `log_dual`（conn + jsonl） | 导出时库是解锁的 |
| `RecoveryExportUsed` | 命令层 `log_dual(None, jsonl)` | 恢复时**没有** `security.db` 连接 |
| `DekRotated` | `lifecycle::rotate_dek` | 解锁态 |
| `AnchorRollbackRejected` | `SecurityDbState::unlock` 失败分支 | 解锁态（`conn` 已打开，只是 anchor 不符） |

### 8.4 下次口令解锁时的补录

`SecurityDbState::unlock` 成功后（§4.5 第 4 步之后）：

```
1. synced_at = meta::get_kv("audit_jsonl_synced_at")（缺失 → 0）
2. records = writer.read_since(synced_at)
3. 对每条 record 且 event ∈ {recovery_export_used, dek_rotated}：
   INSERT INTO security_audit_log (id, event_type, detail, timestamp)
   VALUES ('jsonl-' || at || '-' || event, event, detail, at)
   └─ 已存在（UNIQUE 冲突）则跳过
4. meta::set_kv("audit_jsonl_synced_at", now)
```

约束：

1. **去重键** = `jsonl-{at}-{event}`，天然幂等（同一毫秒同一事件不可能重复发生）。
2. 只补录"恢复态发生、解锁态才可见"的事件类型；其余事件解锁态本来就能直接写库，不需要回放。
3. 补录失败**不阻断解锁**（记 `log::warn!`）。

---

## 9. Step 9 — 命令层与 DTO（补 G9 / G10）

### 9.1 命令增删总表

文件：`src-tauri/src/cmd/vault/recovery.rs`（重写）、`lifecycle.rs`（改）、`entries.rs`（改）。

| 命令 | 处理 | 新签名 |
|---|---|---|
| `vault_init` | **改** | `fn vault_init(app, master_password: String) -> Result<(), VaultErrorDto>`（签名不变，内部改调 `lifecycle::initialize`） |
| `vault_unlock` | **改** | 同上（内部改调 `lifecycle::unlock`）；`vault_show_unlock_window` 内的 `state.unlock(password)`（`lifecycle.rs:89`）同步改 |
| `vault_change_master_password` | **改** | 内部改调 `lifecycle::change_password`；删除 `entries.rs:258-286` 的 emergency 清理、全量重导出与 `RecoveryKeyInvalidated` 审计 |
| `vault_export_recovery_key` | **新增** | `fn vault_export_recovery_key(app, master_password: String) -> Result<RecoveryKeyDto, VaultErrorDto>` |
| `vault_recover_from_paper` | **新增** | `async fn vault_recover_from_paper(app, paper_key: String) -> Result<RecoveryExportDto, VaultErrorDto>` |
| `vault_locate_paper_error` | **新增** | `fn vault_locate_paper_error(paper_key: String) -> Result<Option<PaperErrorPosDto>, VaultErrorDto>` |
| `vault_rotate_dek` | **新增** | `async fn vault_rotate_dek(app) -> Result<RecoveryKeyDto, VaultErrorDto>` |
| `vault_reset_repository` | **新增**（G21） | `fn vault_reset_repository(app, confirm_token: String) -> Result<(), VaultErrorDto>` |
| `vault_generate_recovery_key` | **删除** | — |
| `vault_has_recovery_key` | **删除** | — |
| `vault_verify_vault_meta_integrity` | **删除** | — |
| `vault_disaster_recovery_export` | **删除** | — |

命令注册点同步更新：`src-tauri/src/cmd/vault/mod.rs` 的 `pub use`（已 `pub use recovery::*`，无需改）、`src-tauri/src/vault_plugin.rs` 的 `generate_handler!` 列表。

### 9.2 DTO 定义（补 G10）

文件：`src-tauri/crates/assistant-vault/src/dto/recovery.rs`（新建；`dto/mod.rs` 加 `pub mod recovery;`）。

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
    pub blob: serde_json::Value,    // EntryBlob 的 JSON 形态，前端按既有结构解析
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

`VaultStatusDto` 追加字段（§6.4）：

```rust
#[serde(default)]
pub recovery_backup: Option<RecoveryBackupDto>,   // { dekVersion, confirmedAt }
```

### 9.3 `vault_reset_repository`（G21）

**为什么需要**：设计 §5 第 3 步要求"重置仓库：二次确认 → 删除 `.vault-meta` / `security.db` / `entries/` → 重新初始化"，但没有命令名、没有防误触机制、没有审计事件。这是旧库用户唯一的前进路径，缺它则 §5 的处置闭环不成立。

```rust
fn vault_reset_repository(app: AppHandle, confirm_token: String) -> Result<(), VaultErrorDto>
```

行为：

```
1. `confirm_token != "RESET-VAULT"` ──▶ `Validation`（前端必须显式传字面量）
2. state.lock()（必须先上锁）
3. 备份 .vault-meta → .vault-meta.bak（仅一次，不覆盖已有 .bak）
4. clear_all_biometric_entries(key_store)（§5.8 A）──▶ 失败即中止，不动文件
5. 删除 .vault-meta / security.db（含 -wal / -shm）/ entries/ / audit/
6. 审计 VaultRepositoryReset → JSONL（库已删，只能走 jsonl）
7. 状态复位（conn / dek / db_key = None）
```

约束：

1. **必须先上锁**：解锁态删除会留下打开的文件句柄（Windows 下 `remove_file` 直接失败）。
2. **第 4 步必须早于第 5 步**，且失败即中止（§5.8 A）。漏掉这一步会让重置后的新库继承旧的"已开启生物识别"状态，用户一解锁就收到错误的 `BiometricReset`。
3. **只删这四个路径**，不递归删除仓库目录（`sync/`、配置等不属于密钥体系）。
4. `confirm_token` 是**防止前端误调**的护栏，不是安全机制（前端已二次确认）。字面量写进命令注释。

---

## 10. Step 10 — 前端层（补 G11）

### 10.1 `vault.service.ts` 接口替换

文件：`src/vault/core/vault.service.ts`。

| 方法 | 处理 |
|---|---|
| `generateRecoveryKey()` | **删除** → `exportRecoveryKey(masterPassword: string): Promise<RecoveryKeyInfo>` |
| `hasRecoveryKey()` | **删除** → 改用 `getStatus()` 返回的 `recoveryBackup` |
| `disasterRecoveryExport(key)` | **删除** → `recoverFromPaper(paperKey: string): Promise<RecoveryExportResult>` |
| — | **新增** `locatePaperError(paperKey: string): Promise<PaperErrorPos \| null>` |
| — | **新增** `rotateDek(): Promise<RecoveryKeyInfo>` |
| — | **新增** `resetRepository(): Promise<void>` |

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

export interface PaperErrorPos { group: number; index: number; }
```

`resetRepository()` 内部固定传 `'RESET-VAULT'`；**调用方必须先弹二次确认**（组件职责，见 §10.3）。

### 10.2 错误码联合类型

`src/vault/core/vaultError.ts` 的 `VaultErrorCode` 联合类型同步扩展（8 项，见 §7.4）。注意该联合类型**不是**本文件定义的——它从 `vault.types.ts` 的 barrel 转出，源头是 ts-rs 生成的 `contract/vault.contract.ts`；本文件只维护手写的 `VAULT_ERROR_CODES` 数组（`:5-30`）。新增判定辅助：

```ts
export function needsRepositoryReset(code: VaultErrorCode): boolean {
  return code === 'unsupportedVaultVersion';
}
export function needsDbRepair(code: VaultErrorCode): boolean {
  return code === 'localDbKeyMismatch';
}
```

### 10.3 三个组件

| 组件 | 职责 | 关键约束 |
|---|---|---|
| `RecoveryKeyDialog.tsx` | 展示 5 组纸面、复制、强制"我已抄写"勾选 | 关闭前若未勾选 → 二次确认；**不提供"稍后再说"直接关闭**（设计 §2.7 打印优先） |
| `PaperRecoveryFlow.tsx` | 输入 → `locatePaperError` 定位 → 失败提示 → 导出 | 定位结果只在**校验失败**时展示；格式错不定位 |
| `DbRepairDialog.tsx` | `localDbKeyMismatch` 时给两条路径：输入旧口令修复 / 重建本机库 | 重建路径必须写明"会丢失本机审计历史，条目不受影响" |

`UnsupportedVaultVersion` 的处置入口放在解锁页：提示"此密码箱由旧版本创建，需要重置后才能继续"→ 按钮触发 `resetRepository()`（二次确认 + 输入 `RESET-VAULT` 由组件代填）。

### 10.4 i18n

文件：`src/i18n/locales/{zh-CN,en-US}/vault.json`（或项目现有命名，按实际目录）。

新增 key（**必须成对**）：

```
vault.recovery.export.title / desc / copy / copied / confirmWritten
vault.recovery.paper.inputLabel / inputHint / locateSuccess / locateFailed / checksumFailed / formatFailed
vault.recovery.export.success / skippedCount
vault.rotate.title / desc / confirm / success / biometricReenrollRequired
vault.reset.title / desc / confirm1 / confirm2 / tokenLabel / done
vault.error.unsupportedVaultVersion / localDbKeyMismatch / vaultRollbackDetected
vault.error.vaultManifestMismatch / vaultCorrupted
vault.error.invalidRecoveryKeyFormat / invalidRecoveryKeyChecksum / invalidRecoveryKeyOrMetaTampered
vault.repair.title / desc / oldPasswordLabel / rebuildButton / rebuildWarning
```

约束：**禁止在组件内写中文字面量**（项目既有规则）；`vault.error.*` 的 key 名必须与 camelCase 错误码一一对应（`unsupportedVaultVersion` → `vault.error.unsupportedVaultVersion`）。

---

## 11. 验收标准（补 G13 / G16）

### 11.1 P0-1 前置验证（T-P0）

**T-P0-1：`kdf_iter=1` 是否为 no-op**

```rust
#[test]
fn sqlcipher_kdf_iter_one_is_noop() {
    let key = [0x42u8; 32];
    let dir = tempdir();
    let a = open_with(&dir.join("a.db"), &key, "kdf_iter=1");   // PRAGMA key = x'..'
    a.execute("CREATE TABLE t(v INTEGER)", []).unwrap();
    drop(a);

    let b = open_with(&dir.join("b.db"), &key, "kdf_iter=256000");
    b.execute("CREATE TABLE t(v INTEGER)", []).unwrap();
    drop(b);

    let key_hex = hex::encode(key);
    let ca = Connection::open(&dir.join("a.db")).unwrap();
    ca.execute_batch(&format!("PRAGMA key = \"x'{key_hex}'\";")).unwrap();
    assert!(ca.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get::<_, i64>(0)).is_ok());

    let cb = Connection::open(&dir.join("b.db")).unwrap();
    cb.execute_batch(&format!("PRAGMA key = \"x'{key_hex}'\"; PRAGMA kdf_iter = 1;")).unwrap();
    assert!(cb.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get::<_, i64>(0)).is_ok());
}
```

**判定**：两个断言都通过 → `kdf_iter` 对 raw key 无效（P0-1 成立）。任一失败 → **P0-1 必须回退到"显式设置 `kdf_iter` 并在解锁端镜像设置"**，并在设计文档记录修正。

**T-P0-2：`kdf_iter` 出现在 SQLCIPHER_PRAGMAS 中且去重**

```rust
#[test]
fn pragmas_has_no_duplicate_keys() {
    let keys = SQLCIPHER_PRAGMAS.split(';')
        .filter_map(|s| s.trim().split('=').next())
        .map(str::trim).filter(|s| !s.is_empty())
        .collect::<Vec<_>>();
    let mut uniq = keys.clone(); uniq.sort(); uniq.dedup();
    assert_eq!(keys.len(), uniq.len(), "duplicate PRAGMA keys in SQLCIPHER_PRAGMAS");
}
```

**T-P0-3：`SQLCIPHER_PRAGMAS` 含 `kdf_iter=1`**

```rust
assert!(SQLCIPHER_PRAGMAS.contains("kdf_iter=1"));
```

### 11.2 P0-2 验收

| 编号 | 断言 |
|---|---|
| **T-G1** | `canonical_meta_bytes` 黄金向量：固定 meta → 输出字节与硬编码期望完全一致 |
| **T-G2** | `build_slot_aad` 输出字节序列与 §3.6 逐字节一致（`i9:v1:slot\0dek\0password\0pwd-1\0` + `00..01` 8 字节） |
| **I1** | 新建库：`vault_init` → `dekVersion=1`、`slots.len()=1`、`slot.id="pwd-1"`、`manifest.count=0`、`recoveryBackup=None` |
| **I2** | 改密：`pwd-1` → `pwd-2`；`dekVersion` 不变；条目 `updated_at` 不变；`wrappedDek` 变化；`dbKey` 变化（`security.db` 用新口令可开、旧口令 `LocalDbKeyMismatch`） |
| **I3** | 改密后 `dek` 不变：用改密前的 `dek` 解任一条目成功 |
| **I4** | 解锁链路：口令错 → `InvalidPassword`；`dbKey` 对但 meta 被改 → `VaultRollbackDetected` |
| **I5** | 严格解析：`version=1` 的 meta → `UnsupportedVaultVersion`；`version` 缺失且有 `crypto.masterKeySalt` → `UnsupportedVaultVersion`；`version` 缺失且无特征键 → `VaultCorrupted` |
| **I10** | `UnsupportedVaultVersion` 路径不修改任何文件（`.vault-meta` / `security.db` / `entries/*` 的 mtime 与内容 hash 不变） |
| **I12** | 生物识别 payload 用错误的 `dbKey` → `BiometricReset`（**不得**进入已解锁态） |
| **T-D1** | `VaultErrorCode` 全部变体的 camelCase 集合 == `vaultError.ts` 数组（双向包含） |

### 11.3 P0-3 验收

| 编号 | 断言 |
|---|---|
| **I6** | manifest 的 `count` 与实际条目数一致；删一条 `.enc` 后 `verify_manifest` → `VaultManifestMismatch` |
| **I7** | `decode_paper(encode_paper(dek))` 往返得到同一 `dek`（property test，1000 次随机 `dek`） |
| **I8** | `decode_paper` 的 6 级校验顺序：长度 39 → `InvalidRecoveryKeyFormat`；40 字符但含 `U` 归一化后 CRC 不符 → `InvalidRecoveryKeyChecksum` |
| **I9** | 纸面抄错一位（含 `O/I/L/U` 混淆）→ `locate_checksum_error` 返回**恰好一个**位置，且该位置 == 实际抄错位 |
| **I11** | `open_recovery_session` + `export_all` 得到全部条目明文；**不产生任何 `security.db` 读写**（用只读文件系统断言或 mtime 断言） |
| **T5b** | `locate_checksum_error` 全程不调用 `derive_kek`（用 `#[cfg(test)]` 计数器断言调用次数为 0） |
| **T-N1** | 归一化：`０１２３`、`O/I/L/U`、连字符、全角空格全部正确映射 |

### 11.4 P1 验收

| 编号 | 断言 |
|---|---|
| **I14** | `rotate_dek` 后：`dekVersion+1`、`recoveryBackup=None`、旧纸面解码后 HMAC 失败 → `InvalidRecoveryKeyOrMetaTampered`、新纸面可用、`vaultId` 不变、`dbKey` 不变（`PRAGMA rekey` 未被调用） |
| **I15** | `rotate_dek` 中途失败（模拟第 k 条写失败）→ 仓库保持旧状态（`entries/` 无混合内容、meta 仍为旧 `dekVersion`） |
| **I16** | `rotate_dek` 后生物识别凭据被删除，`restore_keys_from_plaintext` → `BiometricReset` |
| **I17** | `LocalDbKeyMismatch` 两条修复路径各成功一次；`rebuild_local_db` 后 `anchor.revision=0` 且条目全部可读 |
| **T-B1** | `rotate_dek` 时注入 `delete_key("vault-payload")` 失败 → `rotate_dek` 返回错误，且 `dekVersion` **未**递增、`dek` **未**落盘（验证 §5.6 约束 2 的顺序） |
| **T-B2** | `vault_reset_repository` 后 `is_biometric_enrolled()` 返回 **false**，且 keystore 中 4 个 key id 全部不存在（验证 §5.8 A） |
| **T-B3** | `vault_reset_repository` 时注入 `clear_all_biometric_entries` 失败 → 命令返回错误，且 `.vault-meta` / `security.db` / `entries/` **均未被删除**（验证 §5.8 A 的"失败即中止"） |
| **T-B4** | Windows 下把 `vault-sig-hash` 改成任意其他值 → 解锁返回 `BiometricReset`，审计 `BiometricFailed{reason:"sig_changed"}`，且 `restore_keys_from_plaintext` **未被调用**（验证 §5.3.1 第 4 步早于解密，C-15） |
| **T-B5** | 篡改 `vault-payload` 使 GCM 认证失败 → 解锁返回 `BiometricReset`，**不是** `VaultError::Crypto`（验证 §5.3.1 约束 2，C-15） |

> T-B1 / T-B2 / T-B3 通过 mock `BiometricKeyStore` 注入失败。**前提**是 trait 层的 `delete_key` 具备"可失败"语义（§5.8 A-1）——若实现仍恒返回 `Ok`，这三条测试在真实平台上无效。
>
> T-B4 / T-B5 需通过 mock `request_signature` 返回值与直接改写 keystore 中的条目实现；两者都**不得**依赖真实 Windows Hello 弹窗。

### 11.5 回归清单

P0-2 落地后必须全绿：

1. `cargo test`（`src-tauri/crates/assistant-vault`、`assistant-crypto`）
2. `cargo test --test assembly_contract`（cwd = `src-tauri`）
3. 前端 `pnpm test`（vault 相关用例）
4. 手动：新建 → 加条目 → 锁 → 解锁 → 改密 → 锁 → 新口令解锁 → 纸面导出 → 恢复导出

---

## 附录 A：常量与字面量总表

| 常量 | 值 | 位置 |
|---|---|---|
| `DEK_LEN` | `20` | `assistant-crypto/vault_crypto/types.rs` |
| `KDF_SALT_LEN` | `16` | 同上 |
| `KDF_M_DEFAULT` / `T_DEFAULT` / `P_DEFAULT` | `65536` / `3` / `4` | 同上 |
| `WRAPPED_DEK_LEN` | `48` | 同上 |
| `BIOMETRIC_PAYLOAD_LEN` | `52` | `security_db/biometric_ops.rs` |
| `PAPER_CHARS` / `PAPER_GROUPS` / `PAPER_GROUP_LEN` | `40` / `5` / `8` | `recovery_key/mod.rs` |
| `PAPER_VERSION_BYTE` | `0x02` | 同上 |
| `SQLCIPHER_PRAGMAS` | 含 `kdf_iter=1`，无重复键 | `security_db/mod.rs` |
| HKDF info 前缀 | `i9:v1:sqlcipher:` / `i9:v1:entry:` / `i9:v1:meta-integrity` | `assistant-crypto/vault_crypto/mod.rs` |
| AAD 前缀 | `i9:v1:slot` | 同上 |
| 生物识别挑战 | `SHA-256("assistant-vault-biometric-v1")` → 32B（冻结，§5.1.5） | `biometric/windows.rs` |
| 生物识别 HKDF salt / info | `assistant-vault-v1` / `aes-256-gcm-key`（冻结，§5.1.5） | `security_db/biometric_ops.rs` |
| `vault-sig-hash` 哈希对象 | `lowercase_hex(SHA-256(原始签名字节))`（§5.1.5） | 同上 |
| anchor KV 键 | `anchor_meta_hash` / `anchor_dek_version` / `anchor_revision` | `security_db/meta.rs` |
| 审计同步 KV 键 | `audit_jsonl_synced_at` | `security_db/meta.rs` |
| 重置令牌 | `RESET-VAULT` | `cmd/vault/recovery.rs` |
| JSONL 目录 | `<app_data_dir>/audit/` | `audit/jsonl.rs` |

---

## 附录 B：文件改动索引

| 文件 | 动作 | 规格节 |
|---|---|---|
| `assistant-crypto/src/vault_crypto/types.rs` | 新增 `Dek`/`Kek`/`DbKey`/`MetaKey` | §2.1–2.2 |
| `assistant-crypto/src/vault_crypto/mod.rs` | 替换 4 个 v1 派生函数；新增 `derive_*` / `seal_dek` / `open_dek` / `build_slot_aad` / `canonical_meta_bytes` | §3 |
| `assistant-crypto/src/error.rs` | 新增 8 变体、删除 2 变体 | §7.1 |
| `assistant-vault/src/vault_file/types.rs` | 替换 `VaultMetaFile` → `VaultMetaFileV2`；删除 `EmergencyDataFile` | §2.3 |
| `assistant-vault/src/vault_file/mod.rs` | 删除 5 个 v1 函数；新增 6 个 v2 函数；2 处 `derive_entry_key` | §4.2–4.4 |
| `assistant-vault/src/security_db/schema.rs` | **不改**（anchor 走 KV） | §2.4 |
| `assistant-vault/src/security_db/meta.rs` | 删除 `master_key_salt`/`hash_salt` 读写；新增 `Anchor` + 3 个函数 + KV 助手 | §2.4、§4.6 |
| `assistant-vault/src/security_db/mod.rs` | 删 `BOOTSTRAP_SALT`；改 PRAGMA；字段换 `dek`/`db_key`；重写 `init`/`unlock` | §4.5 |
| `assistant-vault/src/security_db/rekey.rs` | 重写 `change_master_password`；新增 `repair_local_db_with_old_password` / `rebuild_local_db` | §4.7–4.8 |
| `assistant-vault/src/security_db/biometric_ops.rs` | payload 52B、key id 改名、探针读、指纹、签名参数冻结、平台入口校验、重包、R8、`clear_all_biometric_entries` | §5、§5.1.5、§5.3.1 |
| `assistant-vault/src/biometric/mod.rs` | trait 增加 ACL 判定方法；`delete_key` 错误语义收紧（not-found → Ok） | §5.7、§5.8 |
| `assistant-vault/src/biometric/{windows,macos,linux}.rs` | `delete_key` 区分 not-found / 真实失败；ACL 判定 | §5.8 A-1、§5.7 |
| `assistant-vault/src/security_db/crypto_ops.rs` / `entries/*.rs` | 6+2+2+1+1 处 `derive_entry_key` 换参 | §4.10 |
| `assistant-vault/src/lifecycle.rs` | **新增**（编排层） | §4.1 |
| `assistant-vault/src/recovery_key/mod.rs` | 删除 v1 恢复；新增纸面编解码 + `RecoverySession` | §3.8、§6 |
| `assistant-vault/src/audit/mod.rs` | `AuditEvent` 增 11 删 2；`log_dual` | §8.1–8.2 |
| `assistant-vault/src/audit/jsonl.rs` | **新增** | §8.2 |
| `assistant-vault/src/dto/error.rs` / `convert.rs` | 增 8 删 2；穷尽 match | §7.2–7.3 |
| `assistant-vault/src/dto/recovery.rs` | **新增** 6 个 DTO | §9.2 |
| `assistant-vault/src/dto/status.rs` | 追加 `recovery_backup` | §6.4 |
| `assistant-vault/src/lib.rs` | 加 `pub mod lifecycle;` | §4.1 |
| `assistant-vault/Cargo.toml` / workspace `Cargo.toml` | 加 `crc = "3"` | §3.8 |
| `src-tauri/src/cmd/vault/lifecycle.rs` | 改 `vault_init`/`vault_unlock`/`vault_show_unlock_window` | §9.1 |
| `src-tauri/src/cmd/vault/entries.rs` | 改 `vault_change_master_password`，删 emergency 清理 | §9.1 |
| `src-tauri/src/cmd/vault/recovery.rs` | 删 4 增 4（含 `vault_reset_repository`） | §9.1、§9.3 |
| `src-tauri/src/vault_plugin.rs` | 更新 `generate_handler!` | §9.1 |
| `src/vault/core/vault.service.ts` | 删 3 增 4 | §10.1 |
| `src/vault/core/vaultError.ts` | 数组增 8 删 2 + 2 个判定函数 | §7.4、§10.2 |
| `src/vault/core/contract/vault.contract.ts` | **重新生成**（ts-rs 导出，勿手改） | §7.4 |
| `src/vault/core/vault.types.ts` | 手写 barrel：删 `exportedAt` 注释 + 追加 3 个新 DTO 导出 | §7.4、§10.1 |
| `src/vault/components/*` | 新增 3 个组件 | §10.3 |
| `src/i18n/locales/*/vault.json` | 新增 ~30 key | §10.4 |
| `assistant-vault/tests/dto_contract.rs` | 新增 T-D1 | §11.2 |
| `assistant-vault/tests/paper_key.rs` | **新增**（I7–I9、T-N1、T5b） | §11.3 |
| `assistant-vault/tests/lifecycle_integration.rs` | **新增**（I1–I6、I10–I17） | §11.2–11.4 |

---

## 附录 C：与设计文档的差异修正记录

| 编号 | 设计文档 | 本规格 | 理由 |
|---|---|---|---|
| **C-1** | §2.3 示例 `dekVersion: 3`；§9.1 I1 断言 `dekVersion=1` | 统一为"初值 1，`rotate_dek` 后递增"；示例的 `3` 标注为示意值 | 两处自相矛盾，必须择一；I1 是可执行断言，优先 |
| **C-2** | §2.6 写 `Corrupted`；§2.7 写 `InvalidRecoveryKeyOrMetaTampered`；§9.2 I4 写 `InvalidRecoveryKey` | 统一为 `VaultCorrupted`（解析失败）与 `InvalidRecoveryKeyOrMetaTampered`（HMAC 失败）两个不同语义的码；`InvalidRecoveryKey` 删除 | 现有 `VaultErrorCode` 无 `Corrupted`；`InvalidRecoveryKey` 在 v1.5 无任何产生点 |
| **C-3** | §6.1 "新增 `anchor_*` 字段"，字面像表列 | anchor 落成 `vault_meta` 表的 3 个 KV 行，**不改 DDL** | `vault_meta` 是 `key/value` 两列结构，加列需 DDL 变更且无收益 |
| **C-4** | §4.5 `RecoverySession` 只含 `dek` | 补 `entries_dir` + `manifest`（均只读无密钥） | 只持 `dek` 无法实现 `export_all` 的磁盘读取 |
| **C-5** | §6.3 "`vault_has_recovery_key` 删除，由 `recoveryBackup` 标记回答" | 标记挂到 `vault_get_status` 返回的 `recoveryBackup` 字段 | 设计未给出前端获取该标记的通道 |
| **C-6** | §6 改动清单无编排层，§6.3 漏 `cmd/vault/lifecycle.rs` | 新增 `assistant-vault/src/lifecycle.rs`；命令层退化为薄封装 | 改密/轮换的跨层顺序不可分割，否则命令层重复实现（现状即如此） |
| **C-7** | §5 "重置仓库" 无命令 | 新增 `vault_reset_repository(confirm_token)` + `VaultRepositoryReset` 审计 | 缺此命令则旧库用户无前进路径，§5 处置闭环不成立 |
| **C-8** | §6.1 列 `derive_entry_key` 17 处 | 复核为 17 处（含 `rekey.rs` 3 处随重写消失，实际净替换 14 处） | 设计把随重写消失的调用点也计入 |
| **C-9** | §2.4 未说明 AAD 在"仅改密"场景的重放弱点 | §3.6 要求把该弱点写进函数注释 | 设计 §2.5 已论证但未要求落到代码注释，易在后续重构中丢失 |
| **C-10** | — | 生物识别 key id 改名后**不做 legacy 清理**（撤销早前拟定的 `LEGACY_KEY_IDS` 方案） | 设计 §5 已确认"无存量用户"，为不存在的用户保留 v1 清理代码即"把 v1 死代码留在仓库里"；开发机残留改为手动清理说明 |
| **C-11** | §5 第 3 步"重置仓库"只删 `.vault-meta` / `security.db` / `entries/` | `vault_reset_repository` 必须**先清空 OS keystore**（§5.8 A） | 漏清则重置后的新库继承"已开启生物识别"状态，用户一解锁收到错误的 `BiometricReset`；设计 I11 只断言"无旧文件残留"，未覆盖 keystore |
| **C-12** | R8 未规定 `delete_key` 失败时如何处置 | `vault-payload` 删除失败**必须中止** `rotate_dek`，且删除须早于 `dek` 落盘（§5.6 约束 1、2） | R8 的安全前提是"凭据已被删除"；静默放过等于前提落空，且新 `dek` 已生效时凭据进入无法自救的中间态 |
| **C-13** | — | Windows / Linux 的 `delete_key` 目前**恒返回 `Ok`**（丢弃所有错误），必须改为区分 not-found 与真实失败（§5.8 A-1） | 否则 §5.6 / §5.8 的"删除失败即中止"在所有平台无法生效，T-B1 / T-B3 退化为恒过的假测试 |
| **C-14** | 设计与规格均未冻结签名挑战值、HKDF 参数、`vault-sig-hash` 哈希对象（只有 key id 表提到 `vault-sig-hash`） | 新增 §5.1.5 冻结全部参数；HKDF salt 沿用 `assistant-vault-v1` **不改名** | 三处在 v1 代码里是硬编码常量，不冻结则"注册用 A、解锁用 B"，症状为 `decrypt_failed`，与凭据变更无法区分；salt 是生物识别派生域标签而非 meta 格式版本，v1.5 无迁移需求，改名无收益只增风险 |
| **C-15** | §5.3 的 8 步流程里**没有** `vault-sig-hash` 检查（该检查实际在 `unlock_windows`，v1 已实现于 `:264-275`） | 新增 §5.3.1 平台解锁入口：Windows 校验 `vault-sig-hash` 且早于解密；两平台解密失败改判 `BiometricReset` | 照 §5.3 原文实现会丢失该检查：凭据重建后密文未变、指纹仍匹配，用户只会看到「解密失败」而非「凭据已变更，请重新注册」，即设计 R10 要消除的错误诊断；`VaultError::Crypto` 也无法让前端给出重新注册引导 |

---

## 附录 D：禁止事项（编译期/评审期硬约束）

1. **禁止 `Dek` / `Kek` / `DbKey` / `MetaKey` 派生 `Clone` / `Copy` / `Serialize` / `Display`**。
2. **禁止任何 v1 → v2 的字段映射、默认值填充、静默降级**（设计 §2.3、§5）。
3. **禁止在 `read_vault_meta_v2` 的 `UnsupportedVaultVersion` 路径上写任何文件**。
4. **禁止 `RecoverySession` 提供任何 DB 句柄访问器**。
5. **禁止 `derive_entry_key` 的 `info` 省略 `i9:v1:entry:` 前缀**。
6. **禁止用 `serde_json::Value` 计算 integrity 规范化字节**（§3.7）。
7. **禁止把 `dek` / `db_key` 写入日志、错误消息、审计 detail、JSONL**。
8. **禁止在 `change_master_password` / `rotate_dek` 中跳过 anchor 更新**。
9. **禁止 `repackage_biometric_payload` 失败时阻断改密主流程**（改密成功优先，生物识别降级为待重注册）。
10. **禁止在组件内硬编码中文文案**（§10.4）。
11. **禁止为 v1 生物识别 key id 写任何读写/清理代码**（§5.1、C-10）。
12. **禁止在 `vault_reset_repository` 中跳过 `clear_all_biometric_entries`**（§5.8 A）。
13. **禁止把 `rotate_dek` 的 `vault-payload` 删除失败降级为 warn**（§5.6 约束 1）。
14. **禁止在 `dek` 落盘之后才删除生物识别凭据**（§5.6 约束 2）。
15. **禁止 `delete_key` 吞掉非 not-found 错误后返回 `Ok`**（§5.8 A-1）。
16. **禁止改动生物识别挑战值、HKDF salt / info、`vault-sig-hash` 的哈希对象**（§5.1.5、C-14）。
17. **禁止在 `unlock_windows` 中先解密再校验 `vault-sig-hash`**（§5.3.1 约束 1、C-15）。
18. **禁止把生物识别 payload 解密失败映射为 `VaultError::Crypto`**（§5.3.1 约束 2、C-15）。
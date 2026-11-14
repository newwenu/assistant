# I9 密码箱密钥体系 — 核心实施规格（v1.5）

> 状态：规格（2026-11-11 职责分派后）
> **本文档职责**：只保留 v1.5 的**核心**——值类型、密码学原语、`.vault-meta` v2、`security.db` 密钥切换、编排层（init/unlock/change/rotate）、核心错误码。
> 其余职责已分派到配套规格（见 §0），本文不再重复定义。
> 上游设计：[I9-vault-key-hierarchy-design.md](./I9-vault-key-hierarchy-design.md)（为什么这样设计、否决了什么、代价记账）。
> 路径约定：全部为仓库相对路径。

---

## 0. 职责分派

### 0.1 规格分工

| 规格 | 负责内容 | 原本文档章节 |
|---|---|---|
| **本文档** | 值类型、`derive_*` / `seal_*` / `open_*`、AAD、规范化、`.vault-meta` v2、`security.db` 解锁/改密/轮换、编排层、核心错误码 | §1–§4、§7.1–7.3、§9.1（生命周期）、§10（核心）、§11.1–11.2（核心） |
| [I9-vault-biometric-spec.md](./I9-vault-biometric-spec.md) | 生物识别 payload、keystore、注册/解锁/重包/吊销、平台 ACL | 原 §5、§9.1（生物识别）、§11.4（T-B*） |
| [I9-vault-recovery-channel-spec.md](./I9-vault-recovery-channel-spec.md) | 纸面编解码、`RecoverySession`、恢复流程、恢复命令/DTO/UI | 原 §2.5、§3.8、§6、§9.2、§10（恢复）、§11.3 |
| [I9-vault-audit-spec.md](./I9-vault-audit-spec.md) | `AuditEvent` 扩展、JSONL 双写、补录、各域事件 | 原 §8 |
| [I9-vault-entry-cipher-spec.md](./I9-vault-entry-cipher-spec.md) | 条目层：DB 列、`EntryFileV2`、`EntryBlobV2`、字段级 AAD | 本文 §4.2/§4.10 的条目部分 |
| [I9-vault-sync-crypto-spec.md](./I9-vault-sync-crypto-spec.md) | 仓库白名单、单一 meta 写入者、pull/push、冲突、`dekVersion` 失配 | 原 G22（本文未展开） |
| [I9-vault-runtime-session-spec.md](./I9-vault-runtime-session-spec.md) | 会话状态机、锁定清理、零化、备份代际、前端驻留 | 原 §4.5 的会话部分 |
| [I9-vault-plaintext-egress-spec.md](./I9-vault-plaintext-egress-spec.md) | 导出/导入、剪贴板、揭示、预览、HIBP、SSH、API key | 本文未覆盖的明文出口 |
| [I9-vault-capability-gating-spec.md](./I9-vault-capability-gating-spec.md) | feature 开关、配置校验、三种版本号、重置仓库、平台能力 | 原 §9.3（重置）、§5.7 |

### 0.2 覆盖缺口去向

| 编号 | 缺口 | 去向 |
|---|---|---|
| **G1** | 密钥类型未定义 | 本文 §2.1–2.2 |
| **G2** | 错误码未落到枚举 | 本文 §7 + 各域规格 |
| **G3** | AAD 字节编码未定 | 本文 §3.6 |
| **G4** | 规范化 JSON 未定 | 本文 §3.7 |
| **G5** | 纸面编码未给字母表与 CRC 参数 | recovery-channel §1 |
| **G6** | manifest 未定义 | 本文 §4.2 |
| **G7** | anchor 未定义 | 本文 §4.6 |
| **G8** | `vaultId` / `dekVersion` 取值规则未定 | 本文 §1.4 |
| **G9** | 命令层只给名字 | 本文 §9 + 各域规格 |
| **G10** | 新命令 DTO 未定义 | recovery-channel §4.2；sync/egress 各自定义 |
| **G11** | 前端接口未定义 | 本文 §10 + recovery-channel §5 |
| **G12** | 生物识别细节未定 | biometric §2–§7 |
| **G13** | `kdf_iter=1` 验证方法未给 | 本文 §11.1 |
| **G14** | 旧库判定未精确 | 本文 §4.3 |
| **G15** | 审计 JSONL 未定 | audit §2 |
| **G16** | 阶段验收标准未给 | 各规格 §测试 |
| **G17** | 设计内部冲突 | 本文附录 C + 各规格差异记录 |
| **G18** | `recoveryBackup` 写入时机 | recovery-channel §3.3 |
| **G19** | `slot.id` 递增规则 | 本文 §1.4 |
| **G20** | 编排层归属未定 | 本文 §4.1 |
| **G21** | 重置仓库命令未定义 | capability-gating §3.2 |
| **G22** | 同步层未覆盖 | sync-crypto-spec（已补齐） |

### 0.3 本文覆盖缺口

经分派后，本文只剩 **11 项**（G1–G4、G6–G9、G13、G14、G17、G19、G20），其余均已落地到域规格。

---

## 1. 实施总览

### 1.1 分层与搭建顺序

```
Step 0  值类型            Dek · Kek · DbKey · MetaKey · VaultId · DekVersion · VaultMetaFileV2 · Anchor
Step 1  密码学原语        derive_kek · derive_db_key · derive_entry_key · derive_meta_key · seal/open_dek · AAD · integrity
Step 2  仓库文件层        .vault-meta v2 严格解析 · 原子写 · manifest · integrity 校验
Step 3  本机库层          schema · meta · init · unlock · change_password · rekey · anchor
Step 4  编排层            initialize / unlock / change_password / rotate_dek
Step 5  生物识别层        → 见 biometric-spec
Step 6  恢复通道层        → 见 recovery-channel-spec
Step 7  错误码层          → 本文 §7 + 各域
Step 8  审计层            → 见 audit-spec
Step 9  命令层与 DTO      本文 §9（生命周期）+ 各域
Step 10 前端层            本文 §10（核心）+ 各域
```

### 1.2 三层性质

| 性质 | Steps | 说明 |
|---|---|---|
| **词汇层** | 0–1 | 定义「密钥长什么样、密文长什么样」。零 IO。没有它写不出任何签名 |
| **数据层** | 2–3 | 唯一读写磁盘的地方（`.vault-meta` 与 `security.db`） |
| **编排层** | 4 | 把两层串成「解锁/改密/轮换」这三个不可分割的事务。**没有它，跨层顺序会散落在命令层里各写一遍** |
| **能力层** | 5–6 | 生物识别与恢复通道。两者都绕开口令，必须显式裁剪权限 |
| **契约层** | 7–8 | 错误码与审计。前端与运维唯一能看到的东西 |
| **连接层** | 9–10 | 命令与前端 |

### 1.3 阶段 ↔ Step 映射

| 阶段（设计 §8） | 本文 Steps | 不可发布窗口 |
|---|---|---|
| **P0-1** | 1.5（仅 `kdf_iter`）+ 1.9（PRAGMA 去重） | — |
| **P0-2** | 0、1（除纸面）、2、3、4、7、9（生命周期）、10（核心） | **P0-2 与 P0-3 之间没有恢复通道，不可发布** |
| **P0-3** | 见 recovery-channel-spec | — |
| **P1-1** | 4.2（manifest）+ 4.6（anchor） | — |
| **P1-2** | 见 biometric-spec | — |
| **P1-3** | 4.9（rotate_dek）+ biometric-spec §9（强制重注册） | — |

### 1.4 取值规则（补 G8 / G19）

| 项 | 规则 | 依据 |
|---|---|---|
| `vaultId` | `Uuid::new_v4().simple().to_string()` → **32 个小写 hex 字符，无连字符**；首次初始化生成，`rotate_dek` 不变 | 设计 §10 D10 |
| `dekVersion` | **初值 1**，`rotate_dek` 后 `+1`，单调不减，`>= 1` | 设计 §9.1 I1；§2.3 示例中的 `3` 只是示意 |
| `slot.id` | `pwd-{n}`，`n` 从 **1** 开始。下一次改密时 `n = 当前 slot.id 解析出的数字 + 1`；解析失败 → `VaultCorrupted` | 设计 §3.2 |
| `slot.createdAt` | **毫秒**（`timestamp_millis`） | `security_audit_log.timestamp` 亦为毫秒 |
| `recoveryBackup.confirmedAt` | **毫秒** | 同上 |
| `manifest.count` | 恒等于 `manifest.entries.len()`；不一致 → `VaultMetaIntegrityFailed` | 设计 §9.2 I6 |
| `anchor.revision` | 从 **0** 起，每成功写一次 `.vault-meta` 后 `+1` | 设计 §2.8 L3 |

---

## 2. 值类型层

### 2.1 `Dek`（20 字节）

位置：`src-tauri/crates/assistant-crypto/src/vault_crypto/types.rs`（与 `MasterKey` 同文件，但**必须是独立 newtype**）。

```rust
pub struct Dek([u8; 20]);

impl Dek {
    pub fn new(bytes: [u8; 20]) -> Self;
    pub fn generate() -> Self;                      // rand::thread_rng().gen()
    pub fn expose(&self) -> &[u8; 20];
    pub fn to_bytes(&self) -> [u8; 20];
    pub fn ct_eq(&self, other: &Dek) -> bool;       // subtle::ConstantTimeEq
}

impl Zeroize for Dek;
impl Drop for Dek;
impl fmt::Debug for Dek                          // "Dek([REDACTED; 20 bytes])"
```

约束：

1. **禁止 `Clone`**。需要传递时用 `&Dek` 或 `Dek::to_bytes()` 重建。
2. **禁止 `Copy` / `Display` / `Serialize`**。序列化 `dek` 只有一条合法出口：`encode_paper`。
3. `ct_eq` 用于 `fingerprint` 比对与任何密钥比较；`PartialEq` 不派生。

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
    pub cipher: String,                     // "aes256Gcm" | "chacha20Poly1305"
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

1. `#[serde(rename = "type")]` 是必须的——`type` 是 Rust 关键字。
2. **不加 `deny_unknown_fields`**：未来加 slot 类型时旧版本应能读到；但**未知枚举值**必须拒绝（§4.3）。
3. 不含 `exportedAt`。v1 的 `exported_at` 随 `export_vault_meta` 删除；v2 的时间字段一律毫秒。

### 2.4 `Anchor`

位置：`src-tauri/crates/assistant-vault/src/security_db/meta.rs`。

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anchor {
    pub meta_hash: String,      // 小写 hex(SHA-256)，见 §4.6
    pub dek_version: u32,
    pub revision: u64,
}
```

**存储形态**：`vault_meta` 表是 `key/value` 两列，因此 anchor 落成**三个 KV 行**，不是三个列：

| key | value |
|---|---|
| `anchor_meta_hash` | 小写 hex |
| `anchor_dek_version` | `u32` 十进制字符串 |
| `anchor_revision` | `u64` 十进制字符串 |

**不改 DDL**，因此 `SCHEMA_VERSION` 保持 `2`。

### 2.5 `RecoverySession`（已迁出）

定义、字段与权限约束见 [I9-vault-recovery-channel-spec.md](./I9-vault-recovery-channel-spec.md) §2。本文不再重复。

---

## 3. 密码学原语层

位置统一：`src-tauri/crates/assistant-crypto/src/vault_crypto/mod.rs`（`assistant-security/src/vault_crypto/mod.rs` 只是 `pub use` 转发，无需改）。

### 3.1 `derive_kek`

```rust
pub fn derive_kek(password: &str, salt: &[u8], m: u32, t: u32, p: u32) -> Result<Kek, VaultError>;
```

行为：

1. `Argon2id`，`Version::V0x13`，输出 **32 字节**直接作为 `KEK`（**不经 HKDF**）。
2. `salt` 长度必须为 **16 字节**；否则 `VaultError::VaultCorrupted`。
3. 参数非法（`m < 8192` / `t < 1` / `p < 1`）→ `VaultError::VaultCorrupted`。**禁止静默用默认参数**。

### 3.2 `derive_db_key`

```rust
pub fn derive_db_key(dek: &Dek, vault_id: &str) -> DbKey;
```

`HKDF-SHA256(salt = 空, ikm = dek, info = "i9:v1:sqlcipher:" + vault_id)`，输出 32 字节。
`vault_id` 为空 → `panic!`（编程错误，不是用户输入）。

> 派生输入是 `dek`，不是 `KEK`（设计评审修正，见附录 C-16）。这样改密才是真 O(1)：`dek` 不变 ⇒ `dbKey` 不变 ⇒ **不需要 `PRAGMA rekey`**，也不会产生多设备的 `LocalDbKeyMismatch`。代价：纸面持有者可推出 `dbKey`，但纸面本就能解全部条目，审计又属于 T3（设计 §0.6），故可接受。

### 3.3 `derive_entry_key`

```rust
pub fn derive_entry_key(dek: &Dek, entry_id: &str) -> PerEntryKey;
```

`HKDF-SHA256(salt = 空, ikm = dek, info = "i9:v1:entry:" + entry_id)`，输出 32 字节。

> 与 v1 的差别：v1 用 `entry_id` 裸做 `info`（无前缀），且 `ikm` 是 32B 口令派生。v1.5 加前缀——这是**密文不兼容**的第二个来源，与不迁移决策自洽。

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
2. GCM 解密失败（含 AAD 不符）→ `VaultError::InvalidPassword`。
3. 禁止把 `nonce` 与 `tag` 拆成独立字段存 meta。

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

1. 每个文本段必须**非空**且**不含 `0x00`**、必须是 ASCII 可打印字符；违反 → `panic!`。
2. 前缀字面量**照抄 `i9:v1:slot`**，不要「顺手改成 v2」。
3. `dek_version` 用 `u64` 而非 `u32` 编码——给未来留位。

**必须写进函数注释的能力边界**：当 `dek` 未变（仅改密）时，新旧 meta 的 AAD 完全一致，整块旧 meta 重放**无法被 AAD 拦截**。这是 C-1 的直接体现。

> 条目文件与字段的 AAD 见 entry-cipher-spec §2.4，不在此重复。

### 3.7 integrity 规范化（补 G4）

**不使用 `serde_json::Value` 做规范化**。改为**专用镜像结构体**，字段顺序固定：

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
| 覆盖范围 | 除 `integrity` 外的**全部**字段 |
| 键序 | 由 `VaultMetaIntegrityView` 的**字段声明顺序**决定 |
| 分隔符 | `serde_json::to_vec` 的紧凑格式（无空格、无换行） |
| 数字 | 全部为整数，无浮点 |
| 缺省 `recoveryBackup` | 输出 `"recoveryBackup":null`，**不省略键** |
| 签名值 | `b64(HMAC-SHA256(metaKey, canonical_meta_bytes))` |

**必须配一条黄金向量测试**（§11.2 T-G1）。

### 3.8 纸面编解码（已迁出）

常量、字节布局、CRC32C 参数、归一化、解码顺序、纠错定位，见 [I9-vault-recovery-channel-spec.md](./I9-vault-recovery-channel-spec.md) §1。本文不再重复。

### 3.9 常量

核心常量见附录 A；纸面与生物识别常量分别在 recovery-channel / biometric 规格中定义。

---

## 4. 仓库文件层、本机库层、编排层

### 4.1 编排层归属（补 G20）

设计 §3.2 / §3.3 的流程跨 `VaultFileManager`（`.vault-meta`）与 `SecurityDbState`（`security.db`）两层，但设计 §6 的改动清单里**没有任何文件负责编排**。

**新增模块**：`src-tauri/crates/assistant-vault/src/lifecycle.rs`（在 `lib.rs` 加 `pub mod lifecycle;`）。

```rust
pub struct VaultPaths<'a> {
    pub repo: &'a VaultFileManager,
    pub state: &'a SecurityDbState,
}

pub fn initialize(paths: VaultPaths<'_>, password: &str, cipher: CipherProvider)
    -> Result<VaultMetaFileV2, VaultError>;

pub fn unlock(paths: VaultPaths<'_>, password: &str)
    -> Result<VaultMetaFileV2, VaultError>;

pub fn change_password(paths: VaultPaths<'_>, current: &str, new: &str)
    -> Result<VaultMetaFileV2, VaultError>;

pub fn rotate_dek(paths: VaultPaths<'_>)
    -> Result<(VaultMetaFileV2, String), VaultError>;   // (新 meta, 新纸面 40 字符)
```

理由：

1. **不可分割性**：改密的「重包 `dek` → 原子写 meta → 更新 anchor」三步跨越两层，任何一层单独暴露都会让命令层重复实现顺序。
2. **可测性**：集成测试直接调这 4 个函数，不需要构造 Tauri `AppHandle`。
3. 命令层退化为薄封装（§9）。
4. **所有权**：`VaultFileManager` 由独立 managed state `VaultRepo`（`Arc<VaultFileManager>`）持有，`SyncEngine` 共享同一实例；命令层只从 `repo_state` 取 repo，**禁止经 `SyncEngine` 转手**（§9.3）。

### 4.2 `VaultFileManager` 改动

文件：`src-tauri/crates/assistant-vault/src/vault_file/mod.rs`。

| 函数 | 处理 |
|---|---|
| `read_vault_meta`（`:260-263`） | **改写**为 `read_vault_meta_v2`（严格解析，见 §4.3） |
| `export_vault_meta`（`:213-258`） | **删除** |
| `read_emergency_data`（`:312-318`） | **删除** |
| `write_emergency_data`（`:320-346`） | **删除** |
| `clear_emergency_data`（`:347-366`） | **删除** |
| `rewrite_vault_meta_with_new_key`（`:367-425`） | **删除** |
| `verify_vault_meta_integrity`（`:265-308`） | **改写**为 `verify_meta_integrity(&self, meta_key: &MetaKey)` |
| `write_entry_file` / `import_entry` | 条目部分见 entry-cipher-spec |
| `write_atomically` | 保留不动 |
| `compute_vault_meta_hmac` / `sign_vault_meta`（`:427-445`） | **删除**，由 §3.7 取代 |
| `VAULT_META_VERSION`（`:27`） | **删除** |

新增：

```rust
impl VaultFileManager {
    pub fn read_vault_meta_v2(&self) -> Result<VaultMetaFileV2, VaultError>;
    pub fn write_vault_meta_v2(&self, meta: &mut VaultMetaFileV2, meta_key: &MetaKey)
        -> Result<(), VaultError>;
    pub fn compute_manifest(&self, dek_version: u32) -> Result<VaultManifest, VaultError>;
    pub fn meta_file_hash(&self) -> Result<String, VaultError>;
    pub fn verify_manifest(&self) -> Result<(), VaultError>;
    pub fn entry_file_path(&self, id: &str) -> PathBuf;
}
```

**`compute_manifest` 的精确语义（补 G6）**：

| 项 | 规定 |
|---|---|
| sha256 覆盖对象 | `entries/{id}.enc` 的**完整文件字节**（含缩进与换行） |
| 编码 | `b64` |
| 排序 | `entries` 按 `id` 字典序升序 |
| `count` | `= entries.len()` |
| 条目级 `dek_version` | 取当前 meta 的 `dekVersion` |
| 计算时机 | ① 条目增/删/改后（同事务）；② `change_password` **不**重算；③ `rotate_dek` 全量重算 |
| 缺失 `.enc` | 该条目从 manifest 移除（不报错） |

### 4.3 `read_vault_meta_v2` 严格解析（补 G14）

判定顺序**不可调换**：

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
   ├─ version 存在且 > 2 ────────────▶ VaultVersionTooNew
   ├─ version 存在但 == 0 或非法 ─────▶ VaultCorrupted
5. 反序列化为 VaultMetaFileV2 失败 ──▶ VaultCorrupted
6. 字段校验（任一失败 → VaultCorrupted）：
   ├─ vault_id 非 32 个小写 hex 字符
   ├─ dek_version == 0
   ├─ cipher 不属于 {"aes256Gcm", "chacha20Poly1305"}（与 VaultConf 枚举一致）
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

1. **禁止任何静默降级**。`version` 缺失**不得**当 2 处理；`slots` 为空**不得**当「未初始化」处理。
2. **不做 v1 字段映射**。识别出 v1 后立即返回，不读任何 `crypto` / `emergency` 内容。
3. **不修改任何文件**。`UnsupportedVaultVersion` 路径不得触碰 `.vault-meta` / `security.db` / `entries/`。

### 4.4 `write_vault_meta_v2`

```
1. 重算 integrity（§3.7）
2. serde_json::to_string_pretty
3. write_atomically（临时文件 + rename）
```

约束：

1. **调用方必须先写好 `dek_version` / `slots` / `manifest` / `recoveryBackup`**；本函数只负责签名与落盘。
2. 落盘成功后由编排层更新 anchor 的 `revision`。
3. **不做「内容相同就跳过」的优化**。v2 每次写都推进 `revision`。

### 4.5 `SecurityDbState` 改动

文件：`src-tauri/crates/assistant-vault/src/security_db/mod.rs`。

| 项 | v1 现状 | v1.5 |
|---|---|---|
| `BOOTSTRAP_SALT`（`:21-24`） | 编译期常量 | **删除** |
| `SQLCIPHER_PRAGMAS`（`:26-27`） | `kdf_iter = 256000` | `kdf_iter = 1`（P0-1） |
| `master_key` | 口令派生 | 由 `VaultSession.dek` 取代 |
| `bootstrap_key` | 引导密钥 | 由 `VaultSession.db_key` 取代 |
| `get_master_key()` | `MutexGuard<Option<MasterKey>>` | `session.dek`（见 runtime-session-spec） |
| `get_bootstrap_key()` | `MutexGuard<Option<MasterKey>>` | `session.db_key` |
| `with_conn_and_key` | `FnOnce(&Connection, &MasterKey)` | `FnOnce(&VaultSession)` |
| `init(&self, password)` | 两阶段 Argon2id | `init(&self, dek: Dek, db_key: DbKey, anchor: &Anchor)` |
| `unlock(&self, password)` | 两阶段 + 第三次 Argon2id | `unlock(&self, session: VaultSession, expected: &AnchorExpectation)` |

> 会话结构、`epoch`、`with_session` 的完整定义见 [I9-vault-runtime-session-spec.md](./I9-vault-runtime-session-spec.md) §2。本文只保留「密钥切换」语义。

`AnchorExpectation`：

```rust
pub struct AnchorExpectation {
    pub meta_hash: String,
    pub dek_version: u32,
}
```

**`init` 行为**：

```
1. security.db 已存在 ──▶ AlreadyExists
2. 主密码强度检查（保留现有逻辑）
3. 建目录 → Connection::open → PRAGMA key = x'db_key' → SQLCIPHER_PRAGMAS
4. 执行 schema::INIT_SQL → meta::ensure_schema_version
5. meta::set_anchor(&conn, anchor)（revision = 0）
6. 写入 session
```

**`unlock` 行为**：

```
1. PRAGMA key + SQLCIPHER_PRAGMAS
2. 探针读：SELECT value FROM vault_meta WHERE key = 'schema_version'
   └─ 失败 ──▶ LocalDbKeyMismatch
3. 读 anchor：
   ├─ anchor_meta_hash != expected.meta_hash ──▶ VaultRollbackDetected
   ├─ anchor_dek_version != expected.dek_version ──▶ VaultRollbackDetected
   └─ anchor 三键缺失 ──▶ 视为「库被重建」，按 §4.7 处理
4. 写入 session
```

约束：

1. **探针读必须存在**。v1 无验证读，错误密钥直到首次业务查询才暴露。
2. 探针读 SQL 照抄恢复码路径的写法，统一为 `SELECT value FROM vault_meta WHERE key = 'schema_version'`。
3. `unlock` 失败时**不得**写入任何 session。

### 4.6 anchor 读写与校验（补 G7）

文件：`src-tauri/crates/assistant-vault/src/security_db/meta.rs`。

```rust
pub fn get_anchor(conn: &Connection) -> Result<Option<Anchor>, String>;
pub fn set_anchor(conn: &Connection, anchor: &Anchor) -> Result<(), String>;
```

**`metaHash` 算法**：`SHA-256` over **`.vault-meta` 文件的完整字节**（含 `integrity`、缩进与换行），输出**小写 hex**。

理由：

- 覆盖整文件 → 任何字段改动（包括 `integrity`）都会改变 hash；
- 不依赖 §3.7 的规范化规则 → anchor 与 integrity 是两条独立检测链；
- 缩进敏感是**有意的**：任何外部工具重排 JSON 都会触发 `VaultRollbackDetected`。

**`revision` 递增规则**：

| 时机 | 动作 |
|---|---|
| `initialize` | `revision = 0` |
| 每次 `write_vault_meta_v2` 成功后 | `revision += 1` |
| `LocalDbRebuilt`（§4.7） | `revision` 重置为 **0** |
| `unlock` | **只读不写** |

**校验顺序**：先探针读（证明 `db_key` 对），再比 anchor（证明 meta 未被回滚）。顺序不可交换。

### 4.7 `LocalDbKeyMismatch` 修复入口

文件：`src-tauri/crates/assistant-vault/src/security_db/rekey.rs`。

```rust
pub fn rebuild_local_db(
    &self, repo: &VaultFileManager, dek: &Dek, db_key: &DbKey, anchor: &Anchor,
) -> Result<(), VaultError>;
```

**触发场景**（`dbKey = HKDF(dek)` 之后）：

1. `rotate_dek` 后本机库仍使用旧 `dek` 派生的 `dbKey`。
2. 在设备 B 上恢复了 `rotate_dek` 之前的 `security.db` 备份。
3. `rotate_dek` 在「条目重加密 → 写 meta → 本机 rekey」之间崩溃，本机库仍是旧钥匙。

**改密不再触发本错误**：`dek` 不变 ⇒ `dbKey` 不变。这是本修正最大的运维收益。

**为什么不再提供「输入旧口令重包」**：旧口令只能配合**旧** `.vault-meta` 解出旧 `dek`；当前 meta 只包着新 `dek`，因此 `dbKey_old = HKDF(dek_old)` 无法从「当前 meta + 旧口令」得到。旧口令路径在 `HKDF(dek)` 下不可行，只保留重建。

**`rebuild_local_db`**：

```
1. 关闭现有 conn，删除 security.db（保留 .bak 一份）
2. 用 db_key 新建库 + INIT_SQL
3. 从 entries/*.enc 重建 vault_entries 索引
4. 从 .audit/*.jsonl 回放审计（audit-spec §4）
5. set_anchor(anchor)，revision = 0
6. 审计 LocalDbRebuilt
```

约束：

1. **重建是唯一路径**，必须始终可用；不得依赖任何历史密钥或历史槽。
2. 重建的代价是 `anchor` 基线重置，**必须在日志与 UI 里写明**。
3. `security.db` 是缓存，**删除前不要求用户导出**。
4. 多设备口径：改密后其余设备**无需修复**；`rotate_dek` 后其余设备各自重建一次（N 台设备 = N 次重建，但只在真吊销时发生）。

### 4.8 `change_master_password` 重写（O(1)）

文件：`rekey.rs`，签名：

```rust
pub fn change_master_password(
    &self, repo: &VaultFileManager, current: &str, new: &str,
) -> Result<VaultMetaFileV2, VaultError>;
```

流程（**顺序不可调换**）：

```
1. read_vault_meta_v2 → slot
2. KEK_old = derive_kek(current, slot.kdf) → dek = open_dek(KEK_old, wrappedDek, AAD)
   └─ GCM 失败 ──▶ InvalidPassword
3. 新 salt(16) → KEK_new = derive_kek(new, salt_new, 同参数)
4. slot' = { id: pwd-{n+1}, kdf: 新 salt, wrappedDek: seal_dek(KEK_new, dek, AAD(slot', dekVersion)), createdAt: now }
5. write_vault_meta_v2（原子写）                        ← 提交点，唯一跨层写入
6. anchor = { meta_hash: 新文件 hash, dek_version 不变, revision += 1 }
7. 清理历史 `security-*.db.bak`（若存在，见 runtime-session-spec §4.3）
8. 返回新 meta
```

要点：

- `dbKey = HKDF(dek)` 与口令无关 → 改密**不 `PRAGMA rekey` 本机库、不重建**。`dek` / `dbKey` 都不变，因此生物识别 payload（`dek‖dbKey`）**无需重包**。
- 真 O(1)：只重包 20 字节 `dek` + 一次原子写；条目与库内加密列一个字节都不动。
- **无跨层崩溃窗口**：全流程只剩 meta 原子写一个提交点（v1.5 早期版本的「rekey 与 meta 之间崩溃」在 `HKDF(dek)` 下消失）。
- 多设备：其余设备用新口令解锁时派生出同一个 `dek` ⇒ 同一个 `dbKey`，**无需任何修复**。

**删除的 v1 逻辑**：`:70-207` 的全量条目重加密段、`:208` 之后的旧 `PRAGMA rekey`、`meta::get_salt("hash_salt")`、`verify_master_password`。

**备份处理**：改密不再产生新 `.bak`；旧 `.bak` 由同一 `dbKey` 加密，只有当前 `dek` 可解，不存在「旧口令可打开」的问题。第 7 步仅为清理历史残留。

### 4.9 `rotate_dek`

`lifecycle::rotate_dek` 流程：

```
1. 删除生物识别凭据（见 biometric-spec §9）：delete_key("vault-payload") 失败 ──▶ 立即返回错误，仓库未动
2. dek_new = Dek::generate()
3. 全量重加密：逐条 read .enc → open(dek_old, entryKey_old) → seal(dek_new, entryKey_new) → 原子替换
4. 库内加密列同样重加密
5. slot' = seal_dek(当前 KEK, dek_new, AAD(slot', dekVersion+1))
6. dek_version += 1；manifest 全量重算；recovery_backup = None
7. write_vault_meta_v2；anchor 更新（revision += 1）        ← 提交点
8. dbKey_new = HKDF(dek_new, vaultId) → PRAGMA rekey 本机库
9. 返回 (新 meta, encode_paper(&dek_new))
```

约束：

1. `dbKey` **随 `dek` 变化** → 必须在 meta 提交后对**本机库** `PRAGMA rekey`。其余设备下次解锁会落入 `LocalDbKeyMismatch`，按 §4.7 重建。第 8 步崩溃不丢数据：本机库是可重建缓存，下次解锁同样走 §4.7。
2. `vaultId` **不变**。
3. 第 3–4 步失败必须**整体回滚**：先写临时目录，全部成功后再批量 rename；**禁止边写边替换**。
4. `recovery_backup` 必须清空。
5. 生物识别凭据删除放在**第 1 步**（理由见 biometric-spec §9 约束 2）。
6. **禁止在持有 `dek` 会话守卫时调用 `init`/`unlock`/`set_session`**——`std::sync::Mutex` 非重入，会自锁死。正确顺序：取守卫 → 完成第 3–8 步 → 显式 `drop(guard)` → 写入新会话。
7. 轮换成功**不自动重注册**生物识别；由 UI 引导重新注册。

### 4.10 `derive_entry_key` 替换点

条目层格式与 AAD 见 entry-cipher-spec；此处只登记机械替换数量：

| 文件 | 处数 |
|---|---|
| `security_db/crypto_ops.rs` | 6 |
| `security_db/rekey.rs` | 3（随 `change_master_password` 重写而消失） |
| `security_db/entries/write.rs` | 2 |
| `security_db/entries/import.rs` | 2 |
| `vault_file/mod.rs` | 2 |
| `security_db/entries/snapshot.rs` | 1 |
| `security_db/entries/deleted.rs` | 1 |

替换后必须全仓搜索 `derive_entry_key(` 确认无 `MasterKey` 实参残留。

---

## 5. 生物识别层（已迁出）

payload 52B、key id、签名/HKDF 参数冻结、fingerprint、`restore_keys_from_plaintext`、平台入口、注册/吊销、R8/R9、keystore 清理，全部见 [I9-vault-biometric-spec.md](./I9-vault-biometric-spec.md)。本文不再重复，仅保留一处交叉依赖：

- §4.9 第 1 步：`rotate_dek` 前删除凭据。

> 改密不再重包生物识别 payload：`dek` / `dbKey` 均不变（biometric-spec §8）。

---

## 6. 恢复通道层（已迁出）

纸面编解码、`RecoverySession`、`open_recovery_session`、`export_all`、`recoveryBackup`、恢复命令与 UI，全部见 [I9-vault-recovery-channel-spec.md](./I9-vault-recovery-channel-spec.md)。本文仅保留：

- §1.3 的 P0-3 阶段映射；
- §4.9 第 8 步返回新纸面。

---

## 7. 错误码层（核心）

### 7.1 `VaultError` 新增变体

文件：`src-tauri/crates/assistant-crypto/src/error.rs`。

```rust
#[error("Unsupported vault version; reset required")]
UnsupportedVaultVersion,

#[error("Vault version is newer than this application; upgrade required")]
VaultVersionTooNew,

#[error("Local vault database key mismatch")]
LocalDbKeyMismatch,

#[error("Vault rollback detected")]
VaultRollbackDetected,

#[error("Vault manifest mismatch")]
VaultManifestMismatch,

#[error("Vault meta is corrupted or unparsable")]
VaultCorrupted(String),
```

恢复/生物识别/同步/条目层的错误码分别在各自规格中定义，本文不重复。

### 7.2 `VaultErrorCode` 新增项

文件：`src-tauri/crates/assistant-vault/src/dto/error.rs`。

| 变体 | camelCase | retryable |
|---|---|---|
| `UnsupportedVaultVersion` | `unsupportedVaultVersion` | 否 |
| `VaultVersionTooNew` | `vaultVersionTooNew` | 否 |
| `LocalDbKeyMismatch` | `localDbKeyMismatch` | 否 |
| `VaultRollbackDetected` | `vaultRollbackDetected` | 否 |
| `VaultManifestMismatch` | `vaultManifestMismatch` | 否 |
| `VaultCorrupted` | `vaultCorrupted` | 否 |

`is_retryable` **保持不变**：只对 `RateLimited` 与 `GitUnavailable` 返回 true。新增项全部 false。

### 7.3 `convert.rs` 映射

`VaultError → VaultErrorDto` 的 `match` 必须**穷尽**（不加 `_ =>` 兜底）：

```rust
VaultError::UnsupportedVaultVersion => (VaultErrorCode::UnsupportedVaultVersion, e.to_string()),
VaultError::VaultVersionTooNew => (VaultErrorCode::VaultVersionTooNew, e.to_string()),
VaultError::LocalDbKeyMismatch => (VaultErrorCode::LocalDbKeyMismatch, e.to_string()),
VaultError::VaultRollbackDetected => (VaultErrorCode::VaultRollbackDetected, e.to_string()),
VaultError::VaultManifestMismatch => (VaultErrorCode::VaultManifestMismatch, e.to_string()),
VaultError::VaultCorrupted(m) => (VaultErrorCode::VaultCorrupted, m.clone()),
```

### 7.4 前端错误码数组

文件：`src/vault/core/vaultError.ts`。

`VAULT_ERROR_CODE_FLAGS` 是手写镜像表，以 `as const satisfies Record<VaultErrorCode, true>` 声明：`VaultErrorCode` 类型由 ts-rs 导出到 `contract/vault.contract.ts`，流程是「改 Rust 枚举 → 重新生成 contract」，手写表漏项或多项都会在 `npm run typecheck` 阶段报错，不再需要人工逐条核对条数。

一致性测试由 `assistant-vault/tests/dto_contract.rs` 断言（§11.2 T-D1）。

---

## 8. 审计层（已迁出）

`AuditEvent` 扩展、JSONL 双写、补录、各域事件清单，全部见 [I9-vault-audit-spec.md](./I9-vault-audit-spec.md)。本文仅保留 §4.5/§4.7/§4.8/§4.9 中已标注的审计调用点。

---

## 9. 命令层（核心）

### 9.1 生命周期命令

文件：`src-tauri/src/cmd/vault/lifecycle.rs`、`entries.rs`。

| 命令 | 处理 | 说明 |
|---|---|---|
| `vault_init` | **改内部** | 签名不变，内部改调 `lifecycle::initialize` |
| `vault_unlock` | **改内部** | 同上；`vault_show_unlock_window` 内的 `unlock` 同步改 |
| `vault_lock` | **改内部** | 改走 `lock_session`（runtime-session-spec §3） |
| `vault_change_master_password` | **改内部** | 改调 `lifecycle::change_password`；删除 emergency 清理、全量重导出、`RecoveryKeyInvalidated` 审计 |
| `vault_rotate_dek` | **新增** | `async fn vault_rotate_dek(app) -> Result<RecoveryKeyDto, VaultErrorDto>`；返回纸面 DTO 由 recovery-channel-spec 定义 |

其余命令分派：

| 命令组 | 去向 |
|---|---|
| 恢复命令 | recovery-channel-spec §4 |
| 生物识别命令 | biometric-spec §12 |
| 导入/导出、揭示、SSH、HIBP | plaintext-egress-spec |
| 同步命令 | sync-crypto-spec |
| 重置仓库 | capability-gating-spec §3.2 |

命令注册点同步更新：`src-tauri/src/vault_plugin.rs` 的 `generate_handler!`。

### 9.2 DTO

核心 DTO 变更：

- 新增命令 `vault_has_recovery_backup`（定义见 recovery-channel-spec §3.3/§4.2）；**不新增**通用 `VaultStatusDto` / `vault_get_status`；
- 生命周期命令不新增 DTO；
- 纸面/恢复 DTO 见 recovery-channel-spec；
- 同步 DTO 见 sync-crypto-spec。

### 9.3 状态注入与仓库所有权（补 C-17）

`VaultFileManager` 是仓库文件层的无状态句柄（`PathBuf` + cipher 闭包），**与 git 同步无关**；`SyncEngine` 只在构造时复用同一实例。所有权与取用入口固定为：

```rust
// src-tauri/src/cmd/vault/mod.rs（应用层；库层不依赖 tauri）
pub struct VaultRepo(pub Arc<VaultFileManager>);
impl VaultRepo { pub fn manager(&self) -> &VaultFileManager { &self.0 } }

pub(crate) fn repo_state(app: &AppHandle) -> Result<tauri::State<'_, VaultRepo>, VaultErrorDto>;

// vault_setup()
let repo = Arc::new(VaultFileManager::new(vault_dir, Box::new(cipher_algo)));
handle.manage(VaultRepo(repo.clone()));
handle.manage(SyncEngine::new_with_repo(repo, sync_config));
```

约束：

1. 编排层函数（§4.1）与生命周期、恢复、生物识别、轮换命令**只从 `repo_state` 取 repo**，禁止经 `SyncEngine::vault_file_manager()` 转手——否则命令层为拿 repo 引入同步引擎依赖，§4.1 理由 3 的「薄封装」失效，且与 capability-gating-spec §1.1 的 feature-off（不创建 `SyncEngine`）冲突。
2. `SyncEngine` 内持 `Arc<VaultFileManager>` 而非自建实例；保留 `SyncEngine::new`（内部委托 `new_with_repo`）与 `vault_file_manager()` 作为过渡入口供既有测试使用，**命令层不得调用**。
3. `tests/command_gate.rs` 的 `STATE_ACCESS` 必须包含 `repo_state(`，使 `AnyState` / `None` 命令的 state 访问判定覆盖新入口。
4. 新增 managed state 必须在本节列出「持有者 + 取用入口 + feature-off 时是否创建」，不得只写构造点。

---

## 10. 前端（核心）

### 10.1 `vault.service.ts`

核心方法：

| 方法 | 处理 |
|---|---|
| `init` / `unlock` / `lock` | 签名不变 |
| `changeMasterPassword(current, new)` | 签名不变 |
| `rotateDek()` | **新增**，返回 `RecoveryKeyInfo` |

恢复/同步/生物识别/导出方法分别在各自规格中定义。

### 10.2 错误码联合类型

`vaultError.ts` 的 `VAULT_ERROR_CODE_FLAGS` 追加 5 项（§7.4）；域规格各自追加自己的错误码。建议增加判定辅助：

```ts
export function needsRepositoryReset(code: VaultErrorCode): boolean {
  return code === 'unsupportedVaultVersion';
}
export function needsDbRepair(code: VaultErrorCode): boolean {
  return code === 'localDbKeyMismatch' || code === 'vaultRollbackDetected';
}
```

`vaultRollbackDetected` 也走重建入口：重建把 anchor 重置为当前仓库状态，是唯一能恢复的路径（提示文案必须写明「重置后回滚不再被检测」）。

### 10.3 组件

核心只要求：

- `UnsupportedVaultVersion` 的处置入口放在解锁页，按钮触发 `resetRepository()`（capability-gating-spec §3.2）；入口旁必须有备份提示，且检测阶段不改动任何文件（§11.2 I10）。
- 重置成功后界面切到初始化页（未初始化态），**不自动建库**；是否初始化由用户另行点击（capability-gating-spec §3.2 第 6 步）。
- `localDbKeyMismatch` / `vaultRollbackDetected` 时展示 `DbRepairDialog`：仅保留重建入口（`rebuild_local_db`），需输入主密码派生当前 `dek` / `dbKey`；不提供旧口令重包路径（理由见 §4.7）。重建会把 anchor 重置为当前仓库状态，因此文案必须写明「重置后仓库回滚不再被检测」。
- `vaultVersionTooNew` 时展示只读提示面板（`VersionTooNewPanel`）：说明「请升级应用」，**不提供重置入口**（否则会删掉更新版本写入的真实数据）。

其余组件归各域规格。

### 10.4 i18n

核心 key：

```
vault.error.locked / invalidPassword / notInitialized / alreadyExists / rateLimited
vault.error.crypto / notFound / entryCorrupted / validation
vault.error.biometricUnavailable / biometricCancelled / biometricFailed / biometricReset
vault.error.invalidKeyFormat / invalidRecoveryKey / recoveryKeyNotSet
vault.error.vaultMetaIntegrityFailed / unsupportedVaultVersion / vaultVersionTooNew
vault.error.localDbKeyMismatch / vaultRollbackDetected / vaultManifestMismatch
vault.error.vaultCorrupted / cancelled / repoNotInitialized / noRemoteConfigured
vault.error.noUpstream / syncConflict / gitUnavailable / keyMismatch
vault.error.historyDecryptFailed / internal
vault.versionTooNew.title / desc / retry
vault.repair.title / desc / rebuildWarning / passwordLabel / rebuildButton
vault.reset.title / desc / backupHint / confirm1 / confirm2 / tokenLabel / proceed / action
```

约束：**禁止在组件内写中文字面量**；`vault.error.*` 与 camelCase 错误码**一一对应**（由 `vaultErrorText.ts` 的 `satisfies Record<VaultErrorCode, I18nPath>` 在编译期强制穷尽），界面文案只取本地化键、不直接展示后端 `message`（后端原文仅作无码兜底）。`vault.reset.backupHint` 承载「重置不可逆，请先自行备份仓库目录」，与 capability-gating-spec §3.2 第 2 步一致。

---

## 11. 验收标准（核心）

### 11.1 P0-1 前置验证（T-P0）

**T-P0-1：`kdf_iter=1` 是否为 no-op**

```rust
#[test]
fn sqlcipher_kdf_iter_one_is_noop() {
    let key = [0x42u8; 32];
    let dir = tempdir();
    let a = open_with(&dir.join("a.db"), &key, "kdf_iter=1");
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

**判定**：两个断言都通过 → P0-1 成立。任一失败 → **P0-1 必须回退到「显式设置 `kdf_iter` 并在解锁端镜像设置」**，并在设计文档记录修正。

**T-P0-2：`SQLCIPHER_PRAGMAS` 无重复键**

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

**T-P0-3**：`assert!(SQLCIPHER_PRAGMAS.contains("kdf_iter=1"));`

### 11.2 P0-2 验收（核心）

| 编号 | 断言 |
|---|---|
| **T-G1** | `canonical_meta_bytes` 黄金向量：固定 meta → 输出字节与硬编码期望完全一致 |
| **T-G2** | `build_slot_aad` 输出字节序列与 §3.6 逐字节一致 |
| **I1** | 新建库：`vault_init` → `dekVersion=1`、`slots.len()=1`、`slot.id="pwd-1"`、`manifest.count=0`、`recoveryBackup=None` |
| **I2** | 改密：`pwd-1` → `pwd-2`；`dekVersion` 不变；条目文件 sha256 不变；`wrappedDek` 变化；`dbKey` **不变**；本机库**未被 rekey** |
| **I3** | 改密后 `dek` 不变：用改密前的 `dek` 解任一条目成功；`dbKey` 亦不变（改密前的 `security.db` 副本仍可解锁） |
| **I4** | 解锁链路：口令错 → `InvalidPassword`；`dbKey` 对但 meta 被改 → `VaultRollbackDetected` |
| **I5** | 严格解析：`version=1` → `UnsupportedVaultVersion`；`version` 缺失且有 `crypto.masterKeySalt` → `UnsupportedVaultVersion`；`version` 缺失且无特征键 → `VaultCorrupted` |
| **I10** | `UnsupportedVaultVersion` 路径不修改任何文件（mtime 与内容 hash 不变） |
| **T-D1** | `VaultErrorCode` 全部变体的 camelCase 集合 == `vaultError.ts` 数组（双向包含） |

### 11.3 P0-3 验收（已迁出）

纸面往返、CRC 定位、恢复会话只读，见 recovery-channel-spec §7。

### 11.4 P1 验收（已迁出）

生物识别见 biometric-spec §14；同步见 sync-crypto-spec §6；运行时见 runtime-session-spec §7；明文出口见 plaintext-egress-spec §8；能力门禁见 capability-gating-spec §8。

### 11.5 回归清单

P0-2 落地后必须全绿：

1. `cargo test`（`assistant-vault`、`assistant-crypto`）
2. `cargo test --test assembly_contract`（cwd = `src-tauri`）
3. 前端 `pnpm test`（vault 相关用例）
4. 手动：新建 → 加条目 → 锁 → 解锁 → 改密 → 锁 → 新口令解锁

---

## 附录 A：核心常量

| 常量 | 值 | 位置 |
|---|---|---|
| `DEK_LEN` | `20` | `assistant-crypto/vault_crypto/types.rs` |
| `KDF_SALT_LEN` | `16` | 同上 |
| `KDF_M_DEFAULT` / `T_DEFAULT` / `P_DEFAULT` | `65536` / `3` / `4` | 同上 |
| `WRAPPED_DEK_LEN` | `48` | 同上 |
| `SQLCIPHER_PRAGMAS` | 含 `kdf_iter=1`，无重复键 | `security_db/mod.rs` |
| HKDF info 前缀 | `i9:v1:sqlcipher:` / `i9:v1:entry:` / `i9:v1:meta-integrity` | `assistant-crypto/vault_crypto/mod.rs` |
| AAD 前缀 | `i9:v1:slot` | 同上 |
| anchor KV 键 | `anchor_meta_hash` / `anchor_dek_version` / `anchor_revision` | `security_db/meta.rs` |
| 审计同步 KV 键 | `audit_jsonl_synced_at` | `security_db/meta.rs` |
| 重置令牌 | `RESET-VAULT` | capability-gating-spec |

纸面常量（`PAPER_*` / `CROCKFORD_ALPHABET`）见 recovery-channel-spec §1；生物识别常量见 biometric-spec §2/§3。

### A.1 实测解锁耗时（M7.9）

测量方式：Windows 开发机、release 构建，`cargo run --release -p assistant-vault --example bench_unlock`（两次独立运行，含 Argon2id、HKDF、SQLCipher 开库三段；下表取两次运行的区间）。

| 环节 | v1 | v1.5 | 说明 |
|---|---|---|---|
| Argon2id(m=65536,t=3,p=4) 单次 | 85–95 ms | 同（单次） | v1.5 只派生一次 `kek` |
| Argon2id 总次数 | 3 次，合计 261–283 ms | 1 次，85–95 ms | 删除 `bootstrap_key` 与 `hash_salt` 两次派生 |
| HKDF(`dek`→`dbKey`) | — | 13–25 µs | 可忽略 |
| SQLCipher 开库（raw key，`kdf_iter=256000`） | 13.1–15.2 ms | — | v1 |
| SQLCipher 开库（raw key，`kdf_iter=1`） | — | 12.9–15.0 ms | v1.5；raw key 下 `kdf_iter` 仅差 0.17–0.21 ms（近乎 no-op） |
| **解锁总计** | **274–298 ms** | **98–108 ms** | 约 2.7–2.8 倍提速 |
| SQLCipher 开库（passphrase，`kdf_iter=256000`） | 101.6–102.0 ms | — | 仅作对照：口令密钥路径才真正受 `kdf_iter` 影响 |

结论：v1.5 的解锁收益主要来自「删除两次 Argon2id」，而不是 `kdf_iter=1`；`kdf_iter=1` 的意义是避免在已由 Argon2id 派生的全熵 `dbKey` 上重复做 PBKDF2。改密耗时另见 §4.6：只重包 `dek`，不再 `PRAGMA rekey`，与 `dekVersion` 无关。

---

## 附录 B：核心文件改动索引

| 文件 | 动作 | 规格节 |
|---|---|---|
| `assistant-crypto/src/vault_crypto/types.rs` | 新增 `Dek`/`Kek`/`DbKey`/`MetaKey` | §2.1–2.2 |
| `assistant-crypto/src/vault_crypto/mod.rs` | 替换 4 个 v1 派生函数；新增 `derive_*` / `seal_dek` / `open_dek` / `build_slot_aad` / `canonical_meta_bytes` | §3 |
| `assistant-crypto/src/error.rs` | 新增 5 个核心变体 | §7.1 |
| `assistant-vault/src/vault_file/types.rs` | 替换 `VaultMetaFile` → `VaultMetaFileV2`；条目类型见 entry-cipher-spec | §2.3 |
| `assistant-vault/src/vault_file/mod.rs` | 删除 5 个 v1 函数；新增 5 个 v2 函数 | §4.2–4.4 |
| `assistant-vault/src/security_db/schema.rs` | **不改** | §2.4 |
| `assistant-vault/src/security_db/meta.rs` | 删除 3 个 v1 KV；新增 `Anchor` + 3 个函数 | §2.4、§4.6 |
| `assistant-vault/src/security_db/mod.rs` | 删 `BOOTSTRAP_SALT`；改 PRAGMA；`init`/`unlock` 改签名 | §4.5 |
| `assistant-vault/src/security_db/rekey.rs` | 重写 `change_master_password`；新增两条修复入口 | §4.7–4.8 |
| `assistant-vault/src/security_db/entries/query.rs` | 删除 `vault_crypto_info`（v1 专用，导出 salt 的接口） | §4.2、sync-crypto-spec §2.2 |
| `assistant-vault/src/security_db/entries/tx.rs` | `with_transaction` 回调由 `&MasterKey` 改为 `&VaultSession` | §4.5、runtime-session-spec §2 |
| `assistant-vault/src/lifecycle.rs` | **新增**（编排层） | §4.1 |
| `assistant-vault/src/lib.rs` | 加 `pub mod lifecycle;` | §4.1 |
| `assistant-vault/src/dto/error.rs` / `convert.rs` | 增 5 个核心码；穷尽 match | §7.2–7.3 |
| `src-tauri/src/cmd/vault/lifecycle.rs` | 改 `vault_init`/`vault_unlock`/`vault_show_unlock_window` | §9.1 |
| `src-tauri/src/cmd/vault/entries.rs` | 改 `vault_change_master_password`；新增 `vault_rotate_dek` | §9.1 |
| `src-tauri/src/cmd/vault/mod.rs` | 新增 `VaultRepo` state + `repo_state()`；`vault_setup` 先建 repo 再注入 `SyncEngine` | §4.1、§9.3 |
| `assistant-vault/src/sync_engine/mod.rs` | `SyncEngine` 内持 `Arc<VaultFileManager>`；新增 `new_with_repo` | §9.3 |
| `assistant-vault/tests/command_gate.rs` | `STATE_ACCESS` 补 `repo_state(` | §9.3 |
| `src-tauri/src/vault_plugin.rs` | 更新 `generate_handler!` | §9.1 |
| `src/vault/core/vault.service.ts` | 新增 `rotateDek` | §10.1 |
| `src/vault/core/vaultError.ts` | 数组增 5 项 + 2 个判定函数 | §7.4、§10.2 |
| `src/vault/core/contract/vault.contract.ts` | **重新生成** | §7.4 |
| `assistant-vault/tests/dto_contract.rs` | 新增 T-D1 | §11.2 |
| `assistant-vault/tests/lifecycle_integration.rs` | **新增**（I1–I5、I10） | §11.2 |

生物识别、恢复、审计、同步、条目、运行时、明文出口的文件改动见各自规格的「文件改动清单」。

---

## 附录 C：与设计文档的差异修正记录（核心）

| 编号 | 设计文档 | 本规格 | 理由 |
|---|---|---|---|
| **C-1** | §2.3 示例 `dekVersion: 3`；§9.1 I1 断言 `dekVersion=1` | 统一为「初值 1，`rotate_dek` 后递增」；示例的 `3` 标注为示意值 | 两处自相矛盾，必须择一；I1 是可执行断言，优先 |
| **C-2** | §2.6 写 `Corrupted`；§2.7 写 `InvalidRecoveryKeyOrMetaTampered`；§9.2 I4 写 `InvalidRecoveryKey` | 核心用 `VaultCorrupted`；恢复用 `InvalidRecoveryKeyOrMetaTampered`；`InvalidRecoveryKey` 删除 | 现有 `VaultErrorCode` 无 `Corrupted`；`InvalidRecoveryKey` 在 v1.5 无产生点 |
| **C-3** | §6.1 "新增 `anchor_*` 字段"，字面像表列 | anchor 落成 `vault_meta` 的 3 个 KV 行，**不改 DDL** | `vault_meta` 是 `key/value` 结构，加列无收益 |
| **C-4** | §4.5 `RecoverySession` 只含 `dek` | 补 `entries_dir` + `manifest`（见 recovery-channel-spec §2） | 只持 `dek` 无法实现 `export_all` 的磁盘读取 |
| **C-5** | §6.3 "`vault_has_recovery_key` 删除，由 `recoveryBackup` 标记回答" | 新增最小命令 `vault_has_recovery_backup`（当前代码无 `vault_get_status`） | 设计未给出前端获取通道，且现状没有可挂载的状态命令 |
| **C-6** | §6 改动清单无编排层，§6.3 漏 `cmd/vault/lifecycle.rs` | 新增 `assistant-vault/src/lifecycle.rs`；命令层退化为薄封装 | 改密/轮换的跨层顺序不可分割 |
| **C-7** | §5 "重置仓库" 无命令 | 新增 `vault_reset_repository`（见 capability-gating-spec §3.2） | 缺此命令则旧库用户无前进路径 |
| **C-8** | §6.1 列 `derive_entry_key` 17 处 | 复核为 17 处（含 `rekey.rs` 3 处随重写消失，净替换 14 处） | 设计把随重写消失的调用点也计入 |
| **C-9** | §2.4 未说明 AAD 在「仅改密」场景的重放弱点 | §3.6 要求把该弱点写进函数注释 | 设计 §2.5 已论证但未要求落到代码注释 |
| **C-10–C-15** | 生物识别 key id、删除语义、参数冻结、平台入口 | 已迁至 [I9-vault-biometric-spec.md](./I9-vault-biometric-spec.md) §2/§3/§6/§9/§11 | 职责分派 |
| **C-16** | §0.2 A3 / §2.4 / §3.2：`dbKey = HKDF(KEK, vaultId)` | 改为 `dbKey = HKDF(dek, vaultId)` | 设计 §0.6/§7.1 自评 A3 的收益≈0（只保护 T3 审计），代价却是改密后 N 台设备各修一次 + 崩溃窗口 + `LocalDbKeyMismatch` 机制。改为从 `dek` 派生后：改密不再碰本机库、无多设备修复、无跨层崩溃窗口，生物识别也无需重包；代价是纸面持有者可推出 `dbKey`（但纸面本就能解全部条目）。`rotate_dek` 时才需要 rekey 本机库，且其本就是 O(n) 全量重加密 |
| **C-17** | §6.3 / §9 未定义 `VaultFileManager` 的持有者；实现上它被塞进 `SyncEngine` 私有字段，命令层只能经 `sync_state()` 转手取 repo | 新增独立 managed state `VaultRepo`（`Arc<VaultFileManager>`），`SyncEngine` 共享同一实例；命令层从 `repo_state` 取 repo（§4.1 理由 4、§9.3） | 编排层签名本就是 `repo: &VaultFileManager`，库层正确、应用层缺「谁持有」；经 `SyncEngine` 转手使「命令层薄封装」（§4.1 理由 3）失效，并与 capability-gating-spec §1.1 的 feature-off（不创建 `SyncEngine`）冲突——vault 核心能力不能挂在可选同步组件下 |

**保留设计的已知取舍（评审记录，暂不修改）**：A2「纸面本体即 `dek`」保持不变。因此「换一张纸」等价于 `rotate_dek`，代价是 O(n) 全量重加密 + `dekVersion + 1` + 全设备重建本机库。评审曾建议改为 recovery slot（换纸 O(1)、`dek` 可回 256 bit），当前决定不采纳，理由是仓库最小化与恢复路径不依赖仓库封装结构。若「纸面轮换」成为高频需求，应按 recovery slot 重新评估。

---

## 附录 D：禁止事项（核心）

1. **禁止 `Dek` / `Kek` / `DbKey` / `MetaKey` 派生 `Clone` / `Copy` / `Serialize` / `Display`**。
2. **禁止任何 v1 → v2 的字段映射、默认值填充、静默降级**。
3. **禁止在 `read_vault_meta_v2` 的 `UnsupportedVaultVersion` 路径上写任何文件**。
4. **禁止 `derive_entry_key` 的 `info` 省略 `i9:v1:entry:` 前缀**。
5. **禁止用 `serde_json::Value` 计算 integrity 规范化字节**。
6. **禁止把 `dek` / `db_key` 写入日志、错误消息、审计 detail、JSONL**。
7. **禁止在 `change_master_password` / `rotate_dek` 中跳过 anchor 更新**。
8. **禁止在持有会话守卫时调用 `init` / `unlock` / `set_session`**（自锁死）。
9. **禁止保留旧口令可打开的数据库备份**（runtime-session-spec §4.3）。

生物识别、恢复、同步、明文出口的专项禁止事项见各自规格。
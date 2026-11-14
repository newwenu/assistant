# I34 保险库 TOTP 配置：数据归属与持久化规格

| 项 | 内容 |
| --- | --- |
| 文档状态 | 设计（待实施） |
| 审计基线 | 主干 `d728ed0` 之后、I33 代码实施完成（待人工验收）的工作区（2026-11-02） |
| 适用范围 | TOTP 配置（`secret` / `digits` / `period` / `algorithm`）的领域模型、加密 payload 格式、持久化字段、IPC 契约、生成时参数来源 |
| 依据 | 对当前主干 `assistant-crypto` / `assistant-vault` / `src-tauri/src/cmd/vault` / `src/vault` 的逐文件审计 |
| 上游规格 | `docs/archived/vault/I32-vault-git-sync-spec.md`（同步语义）；`docs/archived/infra/dto-boundary-analysis.md` §5.3 方案 A（TOTP DTO 归属，已落地） |
| 取代 | `docs/VAULT-DESIGN.md` §3.3 / §5 中的 TOTP 章节（`totp-rs`、`vault_totp.rs`、KeePassXC `Settings` 结构均与实现不符） |
| 后端入口 | `src-tauri/crates/assistant-crypto/src/vault_crypto/totp.rs`、`src-tauri/crates/assistant-vault/src/{shared,dto,types,security_db,vault_file}`、`src-tauri/src/cmd/vault/audit.rs` |
| 前端入口 | `src/vault/core/vault.service.ts`、`src/vault/core/slices/vaultAuditSlice.ts`、`src/vault/detail/TotpDisplay.tsx`、`src/vault/VaultEntryDrawer.tsx` |
| 不在本文件范围 | Steam Guard 5 位编码、HOTP、扫码（图片解码）、一个条目多 TOTP 槽位、`otpauth://` 的 `image` 参数 |

---

## 1. 问题陈述

TOTP 的**值计算**已经完整可用，但**配置作为条目的属性**没有被建模，导致算法参数在"写入—存储—读取—生成"链路上逐段丢失。

| # | 现象 | 代码事实 | 用户影响 |
| --- | --- | --- | --- |
| P1 | `algorithm` 没有任何持久化通道 | SQLite、`EntryBlob`、`VaultEntrySnapshot`、`VaultEntryImport`、`VaultEntryExport`、DTO 均无该字段；`schema.rs` 只有 `encrypted_totp_secret` / `totp_digits` / `totp_period` | 通过 `otpauth://` 导入 SHA256/SHA512 条目后，生成的是 SHA1 验证码，必错 |
| P2 | `totp_digits` / `totp_period` 写了但不读，且常规 CRUD 根本不写 | `security_db/entries/write.rs` 的 INSERT 与 UPDATE 均不含这两列（仅 `import.rs`、`deleted.rs` 写入）；`vault_totp_generate` 从入参取 `unwrap_or(6)` / `unwrap_or(30)`，不查库 | 8 位、60 秒的条目一律按 6 位 / 30 秒生成 |
| P3 | 生成参数由前端决定 | `vault_totp_generate(id, period?, digits?, algorithm?)` 的四个参数全部可选且来自 IPC；前端实际调用只传 `id`，等于全部回落默认值 | UI 与条目数据脱节；IPC 边界可绕过条目配置任意指定算法参数 |
| P4 | 算法以字符串隐式约定 | `cmd/vault/audit.rs` 用 `Some("SHA256") \| Some("sha256")` 手工匹配，其余一律 SHA1；无枚举校验 | 非法值静默降级为 SHA1，错误不可观测 |
| P5 | 类型双重定义且 crypto 仍带线格式 | `TotpAlgorithm` / `TotpResult` / `ParsedOtpAuth` 与 `TotpAlgorithmDto` / `TotpResultDto` / `ParsedOtpAuthDto` 并存；crypto 值类型仍带 `serde(rename_all = "camelCase")` | 改内部字段名即静默改前端契约，编译器无保护（与方案 A 的"crypto 纯值类型"不符） |
| P6 | 时间源内嵌在 crypto | `generate_totp` / `time_remaining` 内部直接取 `SystemTime::now()` | 无法注入固定时间做单测；生成与倒计时无法对齐同一时刻 |
| P7 | 导入导出丢参数 | `VaultEntryExport` / `VaultEntryCsvRow` 只有 `totp_secret`，无 `digits` / `period` / `algorithm` | CSV/JSON 迁移后 TOTP 条目不可用（除非恰为 6/30/SHA1） |
| P8 | 设计文档失真 | `VAULT-DESIGN.md` 写的是 `totp-rs` crate、`vault_totp.rs`、KeePassXC `Settings{key,digits,step,algorithm,encoder}`；`docs/store/` 无任何活跃 TOTP 数据结构记录 | 按文档实施会引入不存在的依赖与错误的类型归属 |

结论：需要把"TOTP 配置"收敛为**一个领域值对象**，并让**生成参数只来自条目自身**。

---

## 2. 当前事实

### 2.1 分层现状

| 层 | 位置 | 当前内容 |
| --- | --- | --- |
| 算法 | `assistant-crypto/src/vault_crypto/totp.rs` | `TotpAlgorithm`、`TotpParams{digits,period,algorithm}`、`TotpResult{code,time_remaining}`、`ParsedOtpAuth`、`generate_totp`、`time_remaining`、`from_otpauth_uri` |
| DTO | `assistant-vault/src/dto/totp.rs` + `dto/convert.rs` | `TotpAlgorithmDto`、`TotpResultDto`、`ParsedOtpAuthDto` 与 3 个 `From` |
| 命令 | `src-tauri/src/cmd/vault/audit.rs` | `vault_totp_generate` / `vault_copy_totp` / `vault_totp_time_remaining` / `vault_totp_parse_uri` |
| 领域类型 | `assistant-vault/src/types/mod.rs` | `VaultEntrySnapshot` / `VaultEntryImport` 持 `totp_secret` + `totp_digits` + `totp_period` |
| 持久化 | `assistant-vault/src/security_db/schema.rs`、`vault_file/types.rs` | 同三字段 |
| 导出 | `assistant-vault/src/types/mod.rs` | `VaultEntryExport` / `VaultEntryCsvRow` 仅 `totp_secret` |
| 前端 | `vault.service.ts`、`vaultAuditSlice.ts`、`VaultEntryDetail.tsx`、`TotpDisplay.tsx`、`VaultEntryDrawer.tsx` | 生成/复制参数可选；详情页 `period={30}` 硬编码；录入只保存 `secret` |

### 2.2 关键写入/读取路径

```
录入  VaultEntryDrawer → vault_totp_parse_uri → 取 secret 丢弃其余参数
写入  VaultEntryCreate/Update（仅 totp_secret）→ write.rs INSERT/UPDATE
       → 只加密 totp_secret；totp_digits/totp_period 不进入 SQL
存储  vault_entries(encrypted_totp_secret, totp_digits DEFAULT 6, totp_period DEFAULT 30)
读取  snapshot.rs 读出三字段（供 .vault 导出/同步）；query.rs 只判断是否为空
生成  vault_totp_generate(id, period?, digits?, algorithm?) → 用入参或默认值
```

`.vault` 文件与 SQLite 是两条独立通路，但都缺 `algorithm`；`write.rs` 与 `query.rs` 是参数丢失的两个断点。

---

## 3. 设计目标 / 非目标

### 3.1 目标

1. TOTP 配置成为**条目的一部分**：`secret` / `digits` / `period` / `algorithm` 四要素共同存入同一个加密 payload。
2. 算法实现留在 `assistant-crypto`，保持**纯值类型、无 serde**；配置与持久化归 `assistant-vault`。
3. 生成与复制命令**只接收 `id`**，参数一律从条目读取。
4. `otpauth://` 解析结果可完整落库，往返无损。
5. **零数据库迁移**：不改 `vault_entries` 表结构、不升 `SCHEMA_VERSION`；旧 TOTP 数据（纯 Base32）一律判为无效，不再兼容，需重新录入。
6. 文档与代码同步：活跃文档只保留本文件这一份 TOTP 数据模型定义。

### 3.2 非目标

- 不引入 `totp-rs` 或任何新依赖。
- 不实现 Steam Guard（需要 `encoder` 概念，见 D2）。
- 不改变条目级加密方式（仍是逐字段加密 + 每条目派生密钥）。
- 不做 TOTP 二维码图片解码；`otpauth://` URI 字符串解析即为上限。

---

## 4. 目标结构

### 4.1 分层职责

| 层 | 职责 | 内容 |
| --- | --- | --- |
| `assistant-crypto` | 只"算" | 算法枚举、纯参数、按注入时间生成、URI 解析；**无 serde、无 DTO、无条目语义** |
| `assistant-vault::shared` | 领域值对象 | `TotpConfig`（含 secret、参数、校验、`code_at`）与 stored payload 编解码 |
| `assistant-vault::types` | 内部快照/导入/导出 | 只持一个 `totp_secret: Option<String>`（字段名保留，值改为解密后的存储 payload） |
| `assistant-vault::dto` | 线格式 | `TotpInputDto`、`TotpCodeDto`、`ParsedOtpAuthDto`；详情参数回显为三个扁平字段 |
| `assistant-vault::security_db` / `vault_file` | 持久化 | 复用既有 `encrypted_totp_secret` / `totp_secret` 字段存 payload，无 schema 变更 |
| `src-tauri/src/cmd/vault/audit.rs` | 编排 | 读条目 → 解密 → 生成 → 审计 / 剪贴板 |
| `src/vault` | 展示与录入 | 只持有 code、剩余秒数、`period`；不再决定算法参数 |

### 4.2 `assistant-crypto`：纯算法

```rust
pub enum TotpAlgorithm { Sha1, Sha256, Sha512 }   // 不派生 Serialize/Deserialize

pub struct TotpSpec {          // 原 TotpParams，改名以区别"条目配置"
    pub digits: u32,
    pub period: u32,
    pub algorithm: TotpAlgorithm,
}

pub struct TotpCode {
    pub code: String,
    pub time_remaining: u32,
}

pub struct ParsedOtpAuthSpec {  // 原 ParsedOtpAuth，去 serde
    pub secret: String,
    pub digits: u32,
    pub period: u32,
    pub algorithm: TotpAlgorithm,
    pub issuer: Option<String>,
    pub account: Option<String>,
}

pub fn generate_at(secret: &[u8], spec: &TotpSpec, unix_time: u64) -> Result<TotpCode, VaultError>;
pub fn parse_otpauth(uri: &str) -> Result<ParsedOtpAuthSpec, VaultError>;
pub fn decode_base32(input: &str) -> Result<Vec<u8>, VaultError>;   // 保持现有导出，生成路径与测试共用
```

要点：

- 错误沿用 crypto 既有 `VaultError`（`Validation` / `Crypto` 变体），不新增 `TotpError`。
- 删除 `TotpResult`、`TotpParams`、`ParsedOtpAuth` 的 serde 派生；线格式只在 `vault::dto` 存在。
- 所有时间来源改为入参 `unix_time`；`TotpCode.time_remaining` 由同一次 `unix_time` 计算，命令层只取一次当前时间，避免生成与倒计时跨窗口。
- `digits` 统一 `clamp(1, 10)`、`period` 统一 `max(1)`，校验只在 crypto 内做一次。
- `TotpAlgorithm` 提供 `as_str()`（`"sha1"` / `"sha256"` / `"sha512"`）与 `FromStr`（大小写不敏感），供持久化层使用；不依赖 serde。

### 4.3 `assistant-vault::shared`：领域配置

```rust
pub struct TotpConfig {
    secret: VaultPlaintext,
    digits: u32,
    period: u32,
    algorithm: TotpAlgorithm,
}

impl TotpConfig {
    pub fn new(secret: VaultPlaintext, digits: u32, period: u32, algorithm: TotpAlgorithm)
        -> Self;                                   // 内部 clamp，与 crypto 口径一致
    pub fn from_parts(secret: &str, digits: u32, period: u32, algorithm: TotpAlgorithm) -> Self;
    pub fn from_stored(stored: &str) -> Option<Self>;  // 只接受 v1 JSON payload；旧纯 secret 返回 None
    pub fn to_stored(&self) -> String;                 // 序列化为存储 payload
    pub fn secret(&self) -> &VaultPlaintext;
    pub fn digits(&self) -> u32;
    pub fn period(&self) -> u32;
    pub fn algorithm(&self) -> TotpAlgorithm;
    pub fn code_at(&self, unix_time: u64) -> Result<TotpCode, VaultError>;
}
```

- `secret` 私有 + `VaultPlaintext`，Drop 时 zeroize；只允许输入 DTO 接收明文，任何输出 DTO 不回显 secret。
- `TotpConfig` **不实现 Serialize / Deserialize**：payload 编解码由 `from_stored` / `to_stored` 显式完成，避免把 zeroize 类型拖进 serde 层。

**存储 payload 格式（v1）**：

```json
{"secret":"JBSWY3DPEHPK3PXP","digits":6,"period":30,"algorithm":"sha1"}
```

- 存储在 `vault_entries.encrypted_totp_secret` 解密后的内容里，以及 `.vault` 文件的 `totpSecret` 字段里。
- 判别规则：只有 `trim` 后以 `{` 开头且能解析为上述结构的内容才算有效；**其余内容（包括旧格式纯 Base32）一律返回 `None`，视为该条目无 TOTP**。
- JSON 解析失败或 `secret` 为空 → `None`，不使用损坏内容生成验证码。
- `algorithm` 取值为 `sha1` / `sha256` / `sha512`，非法值回退 `sha1`；`digits` / `period` 越界按 crypto 口径 clamp。
- 不引入版本号字段：将来若需要 `encoder`，在 JSON 中加可选键即可（缺省视为不支持）。

### 4.4 持久化

#### SQLite（不改表结构）

```
vault_entries
  encrypted_totp_secret TEXT   -- 唯一有效字段：存加密后的 payload（旧值可能是纯 Base32）
  totp_digits    INTEGER ...   -- 保留但弃用，代码不再写入、不再读取
  totp_period    INTEGER ...   -- 保留但弃用，代码不再写入、不再读取
```

- 不新增列、不改 `INIT_SQL`、**不升 `SCHEMA_VERSION`（保持 2）**、不动 `ensure_schema_version`。
- `write.rs` 的 INSERT / UPDATE 只写 `encrypted_totp_secret`（内容为 `TotpConfig::to_stored()` 后的密文）；不再写 `totp_digits` / `totp_period`。
- `snapshot.rs`、`query.rs`、`deleted.rs`、`import.rs` 一律以 `encrypted_totp_secret` 为准；`import.rs` / `deleted.rs` 也不再写 `totp_digits` / `totp_period` 两列。
- `VaultEntrySnapshot` / `VaultEntryImport` 的 `totp_secret` 值改为"解密后的存储 payload"，并删除 `totp_digits` / `totp_period` 字段。
- 旧库直接打开：解密得到纯 Base32 → `from_stored` 返回 `None`，该条目视为无 TOTP；无需任何迁移。
- 详情回显参数（`digits` / `period` / `algorithm`）：`get_entry` 统一走 `snapshot` → `from_stored`；`has_totp` 以 `from_stored` 是否成功为准。

#### `.vault` 文件（同步/灾难恢复）

- `EntryBlob.totp_secret` 字段名保留，内容改为存 payload（与 DB 同一格式）；`totp_digits` / `totp_period` 字段保留但不再参与解析，新写入固定 6 / 30。
- 旧文件里的纯 Base32 在读取时直接判为无效（`from_stored` 返回 `None`），不做参数组装。
- 反向写出：只写 `totp_secret`（payload）；两列写默认值 6 / 30，维持文件字段完整。
- `sync_engine/preview.rs` 的掩码与 `history.rs` 的 `totpSecret` 字段比较继续把该字段整体当敏感值处理。

#### CSV / JSON 导出

对外格式保持人类可读，不暴露 payload：

- `VaultEntryExport` / `VaultEntryCsvRow` 的 `totp_secret` 继续存纯 Base32，新增 `totp_digits`、`totp_period`、`totp_algorithm` 三列。
- 导入时用三列组装 payload；缺列默认 6 / 30 / `sha1`。
- `CsvRow → VaultEntryCreate` 与 emergency 导入构造点同步改为组装 `totp` 对象。

### 4.5 DTO 契约

```rust
// 录入 / 更新（嵌套对象整体替换，避免"secret 改了但 digits 没改"的字段组合歧义）
pub struct TotpInputDto {
    pub secret: Option<String>,       // None = 清除；Some(空串) 视为非法
    pub digits: u32,
    pub period: u32,
    pub algorithm: TotpAlgorithmDto,
}

pub struct TotpCodeDto {
    pub code: String,
    pub time_remaining: u32,
    pub period: u32,                   // 供前端绘制倒计时环，替代硬编码 30
}

// 解析结果保持现状（secret 是录入时唯一允许回前端的场景）
pub struct ParsedOtpAuthDto { secret, digits, period, algorithm, issuer, account }
```

`VaultEntryCreate` / `VaultEntryUpdate`：

- 移除字段 `totp_secret`，新增 `#[ts(optional = nullable)] pub totp: Option<TotpInputDto>`。
- `Create`：`None` = 无 TOTP；`Some { secret: None }` 同样视为无 TOTP。
- `Update`：`None` = 保持不变；`Some { secret: None }` = 清除；`Some { secret: Some(_) }` = 整体替换。

`VaultEntryDetail`：

- 保留 `has_totp`（多处 UI 依赖布尔语义）。
- 新增 `totp_digits` / `totp_period` / `totp_algorithm`（默认 6 / 30 / sha1，仅 `has_totp` 为真时有意义）。

`TotpResultDto` 改名为 `TotpCodeDto`；`TotpAlgorithmDto` 保持不变（ts-rs 导出的枚举值为 `sha1` / `sha256` / `sha512`）。

### 4.6 命令层

| 命令 | 变更后签名 | 说明 |
| --- | --- | --- |
| `vault_totp_generate` | `(id: String) -> TotpCodeDto` | 解密 `totp_secret` → `from_stored` 得到 `TotpConfig` → 注入当前时间生成 |
| `vault_copy_totp` | `(id: String) -> ()` | 同上，写剪贴板并审计 |
| `vault_totp_parse_uri` | `(uri: String) -> ParsedOtpAuthDto` | 不变 |
| `vault_totp_time_remaining` | **删除** | 剩余秒数随 `TotpCodeDto` 返回；倒计时环使用返回的 `period` |

- 命令层不再接受 `period` / `digits` / `algorithm`，从源头消除 P3、P4。
- `vault_plugin.rs` 注册表同步删除 `vault_totp_time_remaining`。
- 同步更新 `src-tauri/src/cmd/vault/audit.rs` 的审计事件细节（保持 `totp:{id}` 格式）。

### 4.7 前端

| 文件 | 变更 |
| --- | --- |
| `vault.service.ts` | `totpGenerate(id)` / `totpCopy(id)` 去掉三个可选参数；删除 `totpTimeRemaining` |
| `vaultAuditSlice.ts` | store 新增 `totpPeriod`；`generateTotp(id)` 无参数；计时器到期后重新调用同一 id；写入 `{ code, timeRemaining, period }` |
| `VaultEntryDetail.tsx` | `TotpDisplay` 的 `period` 改为 store 中的 `totpPeriod`，删除 `period={30}` |
| `TotpDisplay.tsx` | 保持按 `period` / `timeRemaining` 绘制；无数据时兜底 30 |
| `VaultEntryDrawer.tsx` | 表单状态改为 `{ secret, digits, period, algorithm }`；URI 解析后四要素一起填充；提交时组装 `totp` 对象；编辑时从 `selectedEntry` 回填参数 |
| `vault.types.ts` | 重新生成 `vault.contract.ts` 后，把 `TotpResultDto as TotpResult` 的导出别名同步改为 `TotpCodeDto as TotpCode` |

行为约束：`hasTotp` 为真的条目打开详情时只发起一次 `vault_totp_generate`；锁定、切换条目时停止计时器并清空 `totpCode` / `totpPeriod`（沿用现有 `stopTotpTimer`）。

---

## 5. 零迁移兼容矩阵

| 数据来源 | 内容形态 | 解析方式 |
| --- | --- | --- |
| 旧 SQLite 库 | 解密后为纯 Base32 | `from_stored` 返回 `None`，判为无 TOTP，需重新录入 |
| 新 SQLite 写入 | 加密的 JSON payload | `TotpConfig::from_stored` |
| 旧 `.vault` 文件 | `totpSecret` 为纯 Base32，`totpDigits` / `totpPeriod` 被忽略 | 判为无 TOTP |
| 新 `.vault` 文件 | `totpSecret` 为 JSON payload | 直接解析，忽略两字段 |
| 旧 CSV/JSON | 仅纯 secret，无参数列 | 导入时按 6 / 30 / SHA1 组装（导入是主动操作，仍可用） |
| 新 CSV/JSON | 纯 secret + `digits` / `period` / `algorithm` 三列 | 导入时组装 payload |
| 前端 store 初始值 | — | `totpPeriod = 0`，展示层兜底 30 |

全过程不执行任何 SQL、不改 `schema_version`、不复制或重建数据库。旧 TOTP 数据不再生成验证码，也不产生错误提示。

---

## 6. 验收标准

1. `otpauth://` URI（`algorithm=SHA256`、`digits=8`、`period=60`）→ 保存 → 锁定 → 重新解锁 → 详情验证码与 Google Authenticator 一致。
2. `vault_totp_generate` / `vault_copy_totp` 签名只有 `id`；全仓 grep 不到 `totp_time_remaining`、`TotpResultDto`、`TotpParams`。
3. 详情页倒计时环按条目 `period` 绘制；8 位条目显示 8 位。
4. 旧库（`schema_version = 2`）不做任何 SQL 直接打开；旧条目解密为纯 Base32 时 `has_totp = false`，不生成验证码，也不报错。
5. 旧 `.vault` 文件中的纯 Base32 `totpSecret` 被忽略，导入后该条目 `has_totp = false`。
6. `from_stored` 对纯 Base32 与 v1 JSON 两种输入都有单测；JSON 损坏时返回 `None` 而不是用损坏内容生成。
7. CSV/JSON 导出包含 `digits` / `period` / `algorithm`，再导入可还原。
8. `cargo test`（`dto_contract`、`vault_crud`、`vault_file`、`assistant-crypto` 的 `totp` 测试）通过；`cargo clippy` 无新增警告。
9. ts-rs 生成的 `vault.contract.ts` 与前端 `vault.types.ts` 引用链编译通过。

---

## 7. 实施拆分

每个阶段结束时必须可独立编译、可回滚。

| 阶段 | 内容 | 主要文件 |
| --- | --- | --- |
| 1 | crypto 纯化：去 serde、`TotpParams`→`TotpSpec`、`generate_at` 注入时间、删 `TotpResult` / `time_remaining` | `assistant-crypto/src/vault_crypto/totp.rs`、`assistant-crypto/tests/totp.rs` |
| 2 | 领域与持久化：`TotpConfig` + stored payload 编解码、`totp_secret` 单字段贯通、`.vault` blob、CSV/JSON 导出与 `VaultEntryExport ↔ CsvRow` 转换 | `assistant-vault/src/{shared,types,security_db,vault_file}` |
| 3 | DTO 与命令收敛：`TotpInputDto` / `TotpCodeDto`、`From`、命令签名、删除 `vault_totp_time_remaining`、修正 `CsvRow → VaultEntryCreate` 与 emergency 导入构造点 | `assistant-vault/src/{dto,types,sync_engine/emergency.rs}`、`src-tauri/src/cmd/vault/audit.rs`、`src-tauri/src/vault_plugin.rs` |
| 4 | 前端：service / slice / 详情 / 抽屉 / 重新生成契约类型 | `src/vault/**` |
| 5 | 回归：旧 TOTP 数据判无效验证、payload 往返测试、文档回填（`VAULT-DESIGN.md` TOTP 章节改指向本文件） | 测试与文档 |

全过程没有数据库操作：`SCHEMA_VERSION` 保持 2，旧库与旧文件中的纯 Base32 TOTP 数据直接判为无效。

---

## 8. 决策记录

| 编号 | 决策 | 理由 |
| --- | --- | --- |
| D1 | 持久化改为**单加密 payload**（v1 JSON）放进既有 `encrypted_totp_secret` 列；`totp_digits` / `totp_period` 列弃用 | 零 schema 变更即可让四要素一起存取；避免"列与 payload 两处真相"；参数的读取本来就是"解密后整体使用"，不需要按字段查询 |
| D2 | 本次**不支持** Steam Guard 5 位编码 | 需要 `encoder` 概念与额外字段；`digits` 已允许 1–10，后续在 payload 中加可选 `encoder` 键即可扩展 |
| D3 | 生成参数只来自条目，命令不再接受算法参数 | 消除 IPC 边界可篡改参数；`otpauth` 参数与 UI 显示天然一致 |
| D4 | DTO 用**嵌套 `totp` 对象**而非四个扁平字段 | 更新语义是整体替换；扁平字段会产生"只改 digits 不改 secret"的歧义组合 |
| D5 | 剩余秒数继续由服务端计算并随 code 返回，不引入 `expiresAt` | 前端只做每秒递减与到期重取，避免依赖本地时钟偏差；同时返回 `period` 解决倒计时环硬编码 |
| D6 | `TotpConfig` 放在 `assistant-vault::shared`，不放 `types` | `shared` 已是领域共享值类型层（`VaultCategory` / `CustomFieldInput`），dto 与持久化都要引用它 |
| D7 | **零迁移 + 旧值判无效**：不新增列、不升 `SCHEMA_VERSION`、不执行任何 SQL；纯 Base32 旧值直接返回 `None`（视为无 TOTP），需重新录入 | 既有加密 TEXT 列可以承载任意内容；不写兼容分支可避免"旧参数丢失后生成错误码"这类半兼容状态，逻辑最简 |

---

## 9. 与既有文档的关系

| 文档 | 关系 |
| --- | --- |
| `docs/VAULT-DESIGN.md` §3.3 / §5 | 其 TOTP 数据模型（`totp-rs`、`vault_totp.rs`、KeePassXC `Settings`、`vault_totp_time_remaining`）被本文件取代；实施阶段五只做"改指向本文件"，不重写该文档 |
| `docs/archived/infra/dto-boundary-analysis.md` §5.3 方案 A | 本文件沿用其"crypto 纯值类型、DTO 归 vault"结论，并补完当时未涉及的领域值与持久化部分 |
| `docs/archived/vault/I9-vault-spec-3-security-enhance.md` §6.2 | 存档不改；其接口表中的 `vault_totp_time_remaining` 与参数化生成已过时，以本文件为准 |
| `docs/archived/infra/I20-command-simplification-spec.md` | 其命令清单需在实施阶段三同步删除 `vault_totp_time_remaining` 一行 |

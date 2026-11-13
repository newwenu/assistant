# I9 密码箱 密码模型横向对比（当前实现 / 目标设计 / 参考项目）

> 状态：分析文档（2026-10-28；分析基线为 v1，v1.5 已实现，本文保留作为选型论证与历史对照，不再描述现状）
> 关联：`I9-vault-key-hierarchy-design.md`（P2 目标设计：v1.5 `dek` 解耦 + 纸面即密钥）、`I9-vault-spec-1-foundation.md`、`I9-vault-spec-5-ssh-and-recovery.md`
> 范围：**只讨论密码模型**——凭据如何变成密钥、密钥层次、改密/吊销语义、恢复通道权限半径。不含 UI、同步协议、条目格式细节。
> 读法：先看 §0 结论，再按需读 §3 逐方拆解；§6 是本项目的落地判断。

---

## 0. 结论摘要

1. **当前实现（v1）的根本问题不是「恢复密钥设计错了」，而是「没有数据密钥这一层」**：`master_key` 同时扮演「密码派生物」与「条目密钥根」，因此改密必然换密钥，恢复材料必然作废。参考项目中 **KeePassXC 有完全相同的结构缺陷**，而 **Bitwarden/Vaultwarden 与 LUKS 是正确形态**。
2. **参考项目里没有一个直接给出「恢复通道只能拿数据、不能开库」的模型**。LUKS 的 keyslot、BitLocker 的恢复密码、Bitwarden 的 Emergency Access 都是**完整凭据**（能开库 / 能接管账户）。本项目让恢复纸面只承载 `dek`、`dbKey` 由 `KEK` 派生且不落文件，是相对所有参考项目的**唯一原创取舍**，也是最值得保留的一点。
3. **「改密是否吊销旧凭据」在行业里是分裂的**：KeePassXC / ripasso / 当前实现是「改密即吊销」（重加密），LUKS / BitLocker / Bitwarden 是「改密不吊销」（重包）；prs 介于两者之间——`add` 默认重加密、`remove` 却要显式 `--recrypt`，不重加密则旧 recipient 仍可解。P2 选择了「不吊销」并把它变成显式产品语义——这是**有行业依据的选择**，不是设计偷懒。
4. **原 v2 方案有三处参考项目没有暴露、但本项目必须补的缺口**（详见 §6.3）：`master_key_salt` 判据在 v2 失效、生物识别 payload 的换代生命周期未定义、`clear_emergency_data` 在命令层而非库层。三处已在 `I9-vault-key-hierarchy-design.md` 中补上（`vaultId` 判据、`fingerprint`、库层断言）。
5. **本项目比所有参考项目多一个威胁面**：`.vault-meta` 会进入 git 历史。KeePassXC 是单文件、Bitwarden 是服务端权威，都不需要面对「旧封装头可被重放」。因此约束 C-1（只重包无法吊销）是本项目特有约束，不能靠照抄参考项目绕过。

---

## 1. 对比对象

共 12 个模型，分三组：

| 组 | 编号 | 模型 | 为什么纳入 |
|---|---|---|---|
| 本项目 | P1 | 当前实现 v1（`security.db` + `.vault-meta` v1） | 基线 |
| 本项目 | P2 | v1.5 单槽 `dek` 解耦 + 纸面即密钥（目标设计） | 候选方案 |
| 参考项目 | R1 | KeePassXC（KDBX4 `CompositeKey`） | 结构上与 v1 最像，可验证「同病」判断 |
| 参考项目 | R2 | rust-keepass（KDB v1） | 反面教材：KDF 强度不足 |
| 参考项目 | R3 | Vaultwarden / Bitwarden | 结构上与 v1.5 最像，可验证「解耦」判断 |
| 参考项目 | R4 | prs（GPG 多收件人） | 无口令派生的多凭据模型 |
| 参考项目 | R5 | ripasso（pass + git） | git 同步场景下的「无 vault key」模型 |
| 参考项目 | R6a | keyring-core | 只提供 API 与 store 生命周期，把密钥边界外置给 OS |
| 参考项目 | R6b | keyring（v1 兼容门面） | 旧 API 原样保留，实现已拆到 core + 独立 store |
| 参考项目 | R6c | keyring-rs v3 | 编译期 feature 选 store + mock / 自带 store trait |
| 参考项目 | R7 | watchkey-win（Windows Hello） | 生物识别/硬件绑定凭据模型，含可选远程审批备用包裹 |
| 行业基线 | R8 | LUKS / BitLocker | keyslot 模型原型，判断 P2 是否偏离行业 |

> 覆盖说明：参考目录中与密码模型相关的项目共 **9 个**——`3-vault-sync`（keepassxc、prs、rust-keepass、vaultwarden）与 `4-vault-security`（keyring-core、keyring-rs、keyring-rs-v3、ripasso、watchkey-win）。其中 keyring 三件套是同一生态的三代不同形态，拆为 R6a/R6b/R6c 分别描述；R8 不在参考目录内，是行业基线。`1-ai-agent`、`2-desktop-orb`、`5-design-style` 与密码模型无关，不纳入对比。

---

## 2. 速览矩阵

| 模型 | 数据密钥来源 | 凭据→数据密钥路径 | 恢复通道 | 改密是否吊销旧凭据 | 恢复通道能否开库 |
|---|---|---|---|---|---|
| **P1 当前 v1** | 口令派生（MK） | 3×Argon2id（BK/MK/hash） | 恢复密钥包 `BK‖MK` | **是**（重加密 + rekey） | **能**（等价主密码） |
| **P2 目标 v1.5** | 随机 `dek` / `dbKey` | 1×Argon2id(口令) + HKDF | **纸面直接承载 `dek`**（无 slot） | **否**（只重包） | **不能**（`dbKey = HKDF(KEK)`，不在纸面） |
| **R1 KeePassXC** | 口令派生 | 多组件 SHA256 → KDF transform | **无** | **是**（换最终密钥→重加密） | — |
| **R2 rust-keepass** | 口令派生 | AES-256-ECB 多轮 + SHA256 | **无** | **是** | — |
| **R3 Vaultwarden** | 随机 user key | KDF(口令) → 包 `akey` | Emergency Access 包 `akey` | **否**（重包 akey） | **能**（账户接管） |
| **R4 prs** | 无（逐条 GPG） | 无口令派生 | **无**（私钥丢失即数据丢失） | **有条件**（`remove` 须显式 `--recrypt`，否则旧 recipient 仍可解） | — |
| **R5 ripasso** | 无（逐条 GPG） | 无口令派生 | GPG 私钥 | **是**（全量重加密） | — |
| **R6a/b/c keyring 三件套** | OS 凭据库 | 无 KDF，store 由运行时/编译期选定 | OS 账户 / 凭据库 | — | — |
| **R7 watchkey-win** | Hello 签名派生 | HKDF(签名) | 无（companion 是远程审批用的备用包裹，不是恢复凭据） | — | — |
| **R8 LUKS / BitLocker** | 随机 VK / VMK | 每 slot 独立 KDF | 独立 keyslot / 恢复密码 | **否**（重写 slot） | **能**（完整凭据） |

读法：**「改密是否吊销」这一列决定了产品语义；「恢复通道能否开库」这一列决定了泄露爆炸半径。**

---

## 3. 逐方拆解

### 3.1 P1 当前实现 v1

**密钥层次**（`src-tauri/crates/assistant-security/src/security_db/mod.rs`）

```
password ──Argon2id(BOOTSTRAP_SALT 常量, m=64M,t=3,p=4)──▶ BK(256b)
                                                        └─HKDF("sqlcipher")─▶ SQLCipher 主密钥

password ──Argon2id(master_key_salt 随机32B)─────────────▶ MK(256b)
                                                        └─HKDF(info=entry_id)─▶ 条目密钥

password ──Argon2id(hash_salt 随机32B)───────────────────▶ 校验哈希（存 vault_meta）
```

- 三次 Argon2id：`BOOTSTRAP_SALT` 用于开库（`src-tauri/crates/assistant-security/src/security_db/mod.rs:22`、`:122`），`master_key_salt` 用于条目密钥，`hash_salt` 用于口令校验（同文件 `:144-176`）。
- 派生实现：`src-tauri/crates/assistant-security/src/vault_crypto/mod.rs:31-79`。

**恢复密钥**（`recovery_key/mod.rs`）

```
160bit 随机(20B) ──BASE32 4×8──▶ 展示串
     │
     └─Argon2id(salt 16B)──▶ KEK ──AES-256-GCM──▶ 包裹 BK‖MK (64B) ──▶ .vault-meta.emergency{token,salt}
```

- 关键事实：解出来的是 `BK‖MK`，随后直接 `set_conn_and_key`（`src-tauri/crates/assistant-security/src/recovery_key/mod.rs:127-204`）——**与密码解锁路径完全等价**。
- 无校验位（同文件 `:210-234`）。

**生物识别**（`security_db/biometric_ops.rs`）

```
Windows Hello 签名 ──HKDF-SHA256──▶ AES key ──AES-GCM──▶ 包裹 BK‖MK ──▶ OS keystore
```
（`src-tauri/crates/assistant-security/src/security_db/biometric_ops.rs:22-30`、`:237-300`）

**改密**（`security_db/rekey.rs`）

```
逐行重加密 vault_entries 的 encrypted_password / encrypted_notes / encrypted_totp_secret / custom_fields
→ PRAGMA rekey（换 SQLCipher 密钥）
→ 命令层 clear_emergency_data（清 recovery slot）
```
（`src-tauri/crates/assistant-security/src/security_db/rekey.rs:15`、`:80-176`、`:215`；命令层 `src-tauri/src/cmd/vault/entries.rs:189`）

**`.vault-meta` v1 结构**（`vault_file/types.rs:95-123`）

```json
{ "version": 1,
  "crypto":    { "masterKeySalt", "hashSalt", "argon2Params", "defaultCipher" },
  "emergency": { "token", "salt" },
  "integrity": "HMAC-SHA256(master_key, 全字段)" }
```

**一句话定位**：这不是「密码 + 恢复密钥」的双凭据模型，而是**同一把口令派生密钥的三个副本**——密码一份、恢复密钥一份、生物识别一份。

---

### 3.2 P2 目标设计 v1.5（单槽 + 纸面即密钥）

```
纸面：dek(20) = 160 bit ──Crockford Base32 + CRC32C──▶ 40 字符（不进 git、不进本机）

KEK  = Argon2id(password, salt_slot)        同步 slot: password（包裹 dek）
dbKey = HKDF(KEK, "sqlcipher:" ‖ vaultId)   本机库页密钥，不落任何文件
bio  payload = dek(20) ‖ dbKey(32) + SHA256 指纹，存 OS keystore

条目：HKDF(dek, "entry:"‖entry_id)      完整性：HKDF(dek, "meta-integrity")
```

- 封装：`AES-256-GCM(KEK, dek)`，`AAD = "i9:v1:slot" ‖ payloadKind ‖ type ‖ id ‖ dekVersion`（防槽互换 / kind 降级 / 新旧混用）。
- 完整性：`HMAC-SHA256(metaKey, 除 integrity 外全部字段)` + `manifest`（每条目文件 sha256）+ 本机 `anchor`（防删除与回滚）。
- 改密 = 重包 20 字节 + `PRAGMA rekey`（本机页密钥，因 `dbKey` 由 `KEK` 派生）；吊销 = `rotate_dek`。
- 恢复 = 纸面解出 `dek`，直接验 `integrity` 与解条目，**全程不需要 `KEK`、`dbKey`、Argon2id**。
- 应用侧只有 `.vault-meta` + `security.db` 两个文件，**无 `local-meta.json`、无恢复包**。
- 旧库走**进程内直转**（不落明文）+ journal，v1 分支在迁移完成后删除。

**一句话定位**：把「凭据」与「数据密钥」解耦，并**让纸面直接承载数据密钥本体**——恢复通道的能力裁剪因此不靠 UI 约束、不靠 payload 长度、也不靠「钥匙不在仓里」，而靠「`dbKey` 根本不在纸面上」这一物理事实。

**与原 v2 双槽方案的差别**：v2 需要 `local-meta.json` 来包裹随机 `dbKey`、需要 recovery slot 来存第二份 `dek` 封装。v1.5 把 `dbKey` 改为 `HKDF(KEK)`（派生即得，无物可包）、把恢复密钥改为纸面直存 `dek`（无需仓库内封装），于是**额外文件归零**。代价是改密必须 `PRAGMA rekey` 本机库；收益是少一个文件、少一套槽合并规则、少一代历史槽。

---

### 3.3 R1 KeePassXC（KDBX4 `CompositeKey`）

```
口令 / 密钥文件 / YubiKey(challenge-response)  各自 rawKey
        └─ SHA256 拼接 ──▶ composite raw key
             └─ KDF transform（AES-KDF 或 Argon2，参数在 header）──▶ transformedDatabaseKey
                  └─ SHA256(masterSeed ‖ transformed) ──▶ 最终密钥
```

（`references/3-vault-sync/keepassxc/src/keys/CompositeKey.cpp:85-132`、`format/Kdbx4Writer.cpp:45-68`、`format/Kdbx4Reader.cpp:55-84`）

**实际做法**

- **凭据组件**：`PasswordKey`（rawKey = SHA256(口令 UTF-8)）、`FileKey`（XML v1/v2、32B 二进制、64B hex → 32B，兜底为**任意文件整体 SHA-256**）、`YubiKey` challenge-response。`CompositeKey::rawKey` 把各组件 rawKey **按加入顺序拼接后整体 SHA-256**（`CompositeKey.cpp:85-107`）。三者地位平等，**合取**语义，缺一不可。
- **KDF 与版本**：KDBX3 用 AES-KDF（`transformSeed` + 轮数）；KDBX4 用 Argon2d / Argon2id。**challenge-response 的位置随版本不同**：`transform()` 里 KDBX3 走 `kdf.transform(rawKey())`——CR 不参与 transform、在之后才并入哈希（向后兼容）；KDBX4 走 `rawKey(&seed)`——CR 用 KDF seed 作 challenge，**参与** transform（`CompositeKey.cpp:114-134`）。KDF 参数写在 header 里，随库走，可单独升级——升级参数等同重加密。
- **header 与校验**：header 公开，含 `masterSeed`、KDF 参数、`encryptionIV`、inner random stream 等；header 有 **SHA256 + HMAC 双重校验**，凭据错报 `HMAC mismatch`，文件损坏报结构错误，两类失败可区分。
- **改密**：换口令 = 换最终密钥 = **整库重写**（O(n)）；改 KDF 参数同理。
- **恢复**：**没有恢复通道**。密钥文件不是恢复凭据，而是第二个**必要**组件（合取语义），丢任一个即全丢。
- **本机快捷解锁（QuickUnlock）**：Windows Hello / TouchID / Polkit 对随机 challenge 签名，签名的 SHA-256 作为对称密钥，加密该库的密钥材料，challenge 作为 IV 前置存放在一起，按库 UUID 存在**进程内存**中（`m_encryptedKeys`），退出即失。不落文件、不进 OS 凭据库——属于「会话便利」，不是持久凭据，也不是恢复通道。
- **KeeShare（共享子库）**：导出组被写成独立 KDBX；该容器的 `PasswordKey` 取自**组 custom data 里 `KeeShare_Reference` 记录的 `password`（明文存放）**，`type` 可为 `ImportFrom` / `ExportTo` / `SynchronizeWith`（`keeshare/ShareExport.cpp:119`、`keeshare/KeeShare.cpp:82-110`、`KeeShareSettings.h:121-133`）。签名密钥对（`Own`）与总开关（`Active`）在应用配置里，导出数据用 RSA `EMSA3(SHA-256)` 签名。本质是「另开一把副口令的独立库」，仍是合取 / 完整凭据思路。
- **工程约束**：单文件、可随身、可放网盘；无服务端权威，因此没有「撤销」概念，也不面对 git 历史问题。

**定位**：多凭据「合成」做得最干净，但正因为是合成而非封装，**无法做到改密 O(1)，也无法做恢复通道**。它的 QuickUnlock 是参考项目里与本项目生物识别槽最接近的形态，但**只在内存里活着、不跨重启**；本项目 payload 落在 OS keystore 且跨重启，因此必须处理「payload 换代」这件 QuickUnlock 不需要处理的事。

---

### 3.4 R2 rust-keepass（KDB v1）

```
口令 + 可选 keyfile ──SHA256 合成──▶ masterkey
     └─AES-256-ECB 加密 key_transf_rounds 轮（transformSeed 来自 header）──▶ transformed
          └─SHA256(finalSeed ‖ transformed) ──▶ 最终密钥
```

- 即 KeePass 的 **AES-KDF**：非内存硬、无 Argon2，抗 GPU 差（`references/3-vault-sync/rust-keepass/src/kpdb/crypter.rs:362-395`）。
- 只支持 v1，是**KDF 强度演进的反面教材**：参数（轮数）可调，但算法本身缺乏内存硬特性。

**实际做法**

- **密钥合成**：只有口令时 `SHA256(password)`；只有 keyfile 时取 keyfile key；两者都有时 `SHA256(passwordKey ‖ keyfileKey)`（`crypter.rs:147-194`）。
- **keyfile 解析**：只有三种形态，**不支持 KeePass2 XML 密钥文件**——文件恰为 32B 则直接当 key；恰为 64B 且能按 hex 解码则取解码后的 32B；其余（含 64B 但不是合法 hex）**一律按 2048B 分块读完整文件做 SHA256**（`crypter.rs:245-340`）。
- **transform**：以 header 里的 `transformSeed` 为 AES-256-ECB 密钥，迭代 `key_transf_rounds` 轮；最终密钥 = `SHA256(finalSeed ‖ SHA256(transformed))`。
- **改密**：改口令或改轮数 = 换最终密钥 = **整库重写**（O(n)）。
- **恢复**：**没有恢复通道**；keyfile 是第二个必要组件，不是恢复凭据。
- **失效**：口令与 keyfile 都缺时报 `PassErr`；AES-KDF 非内存硬，离线猜测成本远低于 Argon2。

**定位**：证明「KDF 选错，密钥层次再正确也没用」。

---

### 3.5 R3 Vaultwarden / Bitwarden

```
masterKey = KDF(password, salt, iterations)        ← 每用户独立 salt，公开
akey      = Encrypt(masterKey, userSymmetricKey)   ← 随机数据密钥，被包裹
条目      = Encrypt(userSymmetricKey, ...)
```

（`references/3-vault-sync/vaultwarden/src/db/models/user.rs:44-52`、`:201-220`、`api/core/emergency_access.rs`）

**实际做法**

- **客户端 / 服务端分工**：`masterKey` 在客户端派生，salt 是**用户邮箱的小写形式**；KDF 类型、迭代数、内存与并行度都是**每用户字段**，可被组织策略强制。服务端只保存 `password_hash` 与**包裹后的** `akey`，拿不到明文数据密钥。
- **密钥层次**：`masterKey` 包 `akey`（`User.akey`）→ 条目用 `akey` 加密。组织侧另有 RSA 密钥对（`Organization.private_key` / `public_key`），成员关系上有两把包裹：`Membership.akey` 是**组织对称密钥**被**成员公钥**包裹（组织密钥按人分发），`Membership.reset_password_key` 是**成员自己的 user key** 被**组织公钥**包裹（仅在启用管理员重置时存在）。
- **改密**：`set_password` 只更新 `password_hash` 并替换 `akey`（用新 masterKey 重包同一把 user key），**不重加密条目**。
- **管理员重置（另一条路径）**：管理员用组织私钥解开成员 `Membership.reset_password_key` 里那把 `akey`，再用新临时口令派生的 masterKey 重包，**全程不需要旧口令**。
- **Emergency Access 的两级权限**：分 **View** 与 **Takeover** 两类（`atype` 0/1），带 `wait_time_days` 等待期；流程为 邀请 → 接受 → 确认（此时把**包裹后的 `akey`** 存进 `key_encrypted`）→ 发起 → 批准。View 提供只读查看（`/view`、`/password` 端点），Takeover 才是接管：为 grantor 设新口令与新 `akey`，清空 2FA，移除除 Owner 外的组织成员关系。**等待期由后台 job 兜底**：`emergency_request_timeout_job` 在 `recovery_initiated_at + wait_time_days` 到期后自动置为 `RecoveryApproved`，不需要 grantor 手动批准；另有 `emergency_notification_reminder_job` 持续发提醒邮件。
- **撤销**：删授权行即可撤销；但服务端**备份里的旧包裹**仍可解开——这是「旧封装可重放」，与本项目 C-1 同源，只是发生在服务端备份而非 git 历史。
- **工程约束**：服务端是权威，多设备天然；可自托管，但离线不可用。

**定位**：**「数据密钥与密码解耦」的教科书实现**；但它的恢复通道是**账户接管**——权限半径比 I9 大一个量级（能改密码、能清 2FA）。它证明了解耦与恢复通道可以共存，代价是恢复凭据必须是**完整凭据**；而 View 类型说明「只读」是**服务端授权**做的，不是密钥结构做的。

---

### 3.6 R4 prs（GPG 多收件人）

- 只有一个 **store 根目录下的 `.gpg-id`**（`store.root.join(".gpg-id")`），列出全部 recipient 指纹；**子目录 `.gpg-id` 尚不支持**（README 已列为 open issue）。每条记录用 GPG 加密给**全部** recipient（`references/3-vault-sync/prs/lib/src/crypto/store.rs:13-45`、`crypto/recipients.rs`）。
- 无口令派生、无 vault key；协议枚举目前只有 `Proto::Gpg`（`crypto/mod.rs:30`）。

**实际做法**

- **凭据形态**：没有「库口令」这一层，凭据就是 **GPG 私钥**；每条 secret 是独立的 GPG 文件，加密给 `.gpg-id` 里的**全部** recipient。多机器场景官方建议**每台机器一把独立 GPG 密钥**，而不是共享同一把私钥。
- **公钥随仓库走**：收件人公钥会被同步进仓库内的 `.public-keys/` 目录（`store_sync_public_key_files`：删除已移除收件人的公钥文件、补入新收件人的），因此**不依赖 keyserver**也能在另一台机器上解密与重加密。
- **解锁**：无会话主密钥，每次读条目都走 gpg-agent（口令缓存在 agent 里）；因此不存在「一次解锁全部」的对象，也就没有 slot 抽象。
- **改密**：不适用（没有库口令）。换一把自己的私钥 = 换 recipient。
- **增删 recipient**：`recipients add` / `generate` 默认**顺带重加密全部条目**（可 `--no-recrypt` 跳过）；`recipients remove` 反过来需要显式 `--recrypt` 才会重加密；另有 `housekeeping recrypt`（`--all`）做手工全量重加密。
- **恢复**：没有恢复通道；私钥丢失即数据丢失。**Tomb** 只是把整个 store 目录装进加密容器（仅 Linux），由 prs 自动 open/close/resize，防的是目录元数据泄露，不提供恢复。
- **失效与可观测性**：移除 recipient 却未重加密 → 旧密文仍可被移除者解开；即便重加密，**git 历史里的旧密文依然在**——与本项目 C-1 同类，只是粒度是条目而不是封装头。
- **工程约束**：与 `pass` 文件结构兼容，可互操作；依赖外部 GPG / gpgme 与 gpg-agent；git 同步可选；内置历史跟踪与回滚工具。

**定位**：天然的「多凭据 OR 语义」，但**吊销必然 O(n)**，且没有「同一用户多设备不同凭据」的抽象；同时它也是「git 历史使吊销不彻底」的现成例证。

---

### 3.7 R5 ripasso（pass + git）

- 每条 = 一个 GPG 加密文件，目录即 git 仓库；提交签名可验证；口令由 gpg-agent 缓存（`references/4-vault-security/ripasso/crates/ripasso/src/crypto.rs`、`pass.rs`）。
- **没有 vault key**：不存在「一次解锁全部」的会话密钥，每条都要 gpg 解密。

**实际做法**

- **凭据形态**：与 prs 同源的 `pass` 格式——条目=文件、`.gpg-id` 决定 recipient、目录=git 仓库；凭据是 GPG 私钥，解锁靠 gpg-agent。
- **加解密粒度**：逐条解密，没有库级数据密钥；因此「会话缓存」只能是 gpg-agent 的私钥缓存，而不是 vault 会话密钥。
- **改密**：无库口令可改；换私钥即换 recipient。
- **增删 recipient**：`add_recipient` / `remove_recipient` **无条件**调用 `reencrypt_all_password_entries`——先逐条 `secret()` 解密、再 `update_internal()` 重加密，全部完成后 `add_and_commit` 一次提交，提交信息为 `Reencrypt password store with new GPG ids 0x…`；`remove_recipient` 在重加密失败时会把 `.gpg-id` 回滚成原内容。没有 `--no-recrypt` 这类跳过开关。
- **签名校验**：`valid_gpg_signing_keys` 非空时，读取与写入 `.gpg-id` 都会校验其 GPG 签名，防止有人偷偷改收件人集合。
- **恢复**：没有恢复通道；git 只是版本历史，不是恢复凭据。
- **失效与可观测性**：git 历史保留旧密文（移除 recipient 前的版本仍可被其解开）；冲突以文件冲突呈现，可读性依赖 UI。
- **工程约束**：与 git 天然契合；但无统一数据密钥层，无法做 slot、无法做 O(1) 改密。

**定位**：与 git 同步天然契合（条目=文件，冲突=文件冲突），代价是**没有统一数据密钥层**，因此无法做 slot、无法做 O(1) 改密。

---

### 3.8 R6a keyring-core

- 把密钥交给 OS 凭据库（Windows Credential Manager / macOS Keychain / Secret Service），应用只持有 `service + user` 定位符（`references/4-vault-security/keyring-core/README.md`）。
- 无自研 KDF，安全边界完全交给 OS。

**实际做法**

- **无默认 feature**：`keyring-core` 不内置任何平台 store，只有可选的 `sample` feature（文件持久化的示例 store）。平台 store 由应用另外依赖具体 crate 提供。
- **生命周期由应用管**：启动时 `set_default_store(store)` 设为默认、退出时 `unset_default_store()` 释放——store 选择从**编译期挪到运行时**，这是相对 keyring v3 的核心 API 变化。
- **数据类型**：`set_password` / `get_password`（字符串）与 `set_secret` / `get_secret`（二进制）并列；对本项目而言正好能放加密后的 payload。
- **自带两个跨平台测试 store**：`mock`（无持久化，可同时 mock 成功与**注入错误**）与 `sample`（文件持久化）；两者都明确标注「不安全、不可用于生产」。
- **Entry 不再暴露底层 credential**：`get_credential` 无底层对象时报 `NoEntry`，有则返回**包裹**它的 `Entry`；`Ambiguous` 也改成返回 wrapper entry 列表（仓库内 `ambiguity` 示例演示了不用 credential 处理歧义）。
- **`new_with_target` → `new_with_modifiers`**：`target` 变成 modifiers map 里的一个可选键，是否接受取决于具体 store。
- **新增 `Entry::search`**：接收搜索规格，由 store 决定是否实现（源自社区的 `keyring-search` crate）。
- **安全边界**：不做 KDF、不做封装层次，**保护强度 = OS 凭据库保护强度**（Windows 上是 DPAPI 保护的凭据，随登录用户解密）。因此它的威胁模型里没有「口令」这一项。
- **缺失项**：没有版本、没有轮换、没有多凭据合并语义——这些都得由上层自己实现（本项目即由 vault 层补）。

**定位**：适合作为**生物识别槽的存储后端**（本项目 `BiometricKeyStore` 就是这一类），不适合作为 vault 主密钥的唯一来源（OS 失陷即全失，且无法跨设备恢复）。

---

### 3.9 R6b keyring（v1 兼容门面）

- 生态当前的入口 crate。默认 **`v1` feature** 提供与原 v1 完全相同的 API（`set_password` / `set_secret` / `get_*` / `delete_credential`），**实现已下沉到 keyring-core 与各独立 store crate**；调用方一行不用改（`references/4-vault-security/keyring-rs/README.md`）。
- 另有 **`cli` feature**：一整套示例代码 + 一大票 credential store。README 明确劝阻应用**直接**以 `cli` feature 依赖本 crate（会拖进一堆用不到的 store 与库），正确姿势是依赖 `keyring-core` + 具体 store crate，再从 `cli` 模块里**抄**需要的装配代码。

**实际做法**

- **生态分层**：库/API 部分在 `keyring-core`，store 各自独立成 crate；本 crate 只做「v1 API 兼容层 + 示例集合」。历史沿革：最初由 Walther Chen 写成「API + store 一体」，现由 Dan Brotsky 维护并完成拆分。
- **示例与验证工具**（都在 `cli` 模块内）：`keyring-cli`（一次一个 keyring 调用，二进制以 base64 进出）、Python 绑定（PyO3，PyPI 上的 `rust-native-keyring`）、Tauri 2.0 跨平台 GUI（可在各平台原生查看器里对照）。
- **三个示例应用**：`unit test`（对该平台所有可用 store 跑同一套单测）、`leak test`（检查 secret 是否残留在被 drop 的堆数据里，可拿 `keyring-core::sample` store 做对照看「泄漏长什么样」）、`set-or-clear-three`（造/删三个示例凭据，方便在系统原生凭据查看器里观察）。
- **store 作者入口**：新 store 被鼓励向 `cli` 模块提交 connector，从而同时进入示例与测试应用。

**定位**：证明「换架构不必改调用方」——API 冻结、实现外置。对本项目的意义是：生物识别槽若要接 OS 凭据库，可以把 `set_secret` / `get_secret` 这层直接复用，而把版本与轮换留在 vault 层。

---

### 3.10 R6c keyring-rs v3

- **编译期**用 feature 逐 store 选择：`apple-native`（macOS/iOS Keychain）、`windows-native`（Credential Manager）、`sync-secret-service`（dbus）、`async-secret-service`、keyutils、以及 `vendored`（静态链接 dbus）。一个构建只能有一个默认后端（`references/4-vault-security/keyring-rs-v3/README.md`）。
- **回退规则**：若某平台没有任何「已启用且受支持」的 store，则该平台的默认后端变成 **`mock`**（mock 永远存在）。Linux 另有 Secret Service + keyutils 的组合后端。
- 支持二进制 secret；提供「自带 store」trait；内置 **mock store**（跨平台，可 mock 成功与错误，用于 CI）。

**实际做法**

- **v2 → v3 的 API 变化**：新增 `set_secret` / `get_secret`（二进制）；`delete_password` 改名 `delete_credential`；**取消默认 feature 集**，必须显式声明要哪些 store；数据格式**完全向前兼容**；MSRV 提到 1.75。
- **Secret Service 恒为同步阻塞**：即使底层用 zbus 的 async 接口，本 crate 也只用 blocking 接口；README 建议直接用 `sync-secret-service`（代价是需要 `libdbus`，可用 `vendored` 静态链接），若确要 async 则须指定同一 runtime 并**单独开线程**调用，否则已知会死锁。
- **Windows store 不保证调用顺序**：同一凭据必须只从一个线程访问，需要顺序的操作也要在同一线程内完成。
- **macOS / iOS 不允许空 service / user 名**：空字段在查找时被当作通配符。

**定位**：证明「store 选择放在编译期」的代价——同一份代码在不同平台行为不同，测试也只能编译期替换。keyring-core 正是为把这一步挪到运行时而拆出来的。本项目生物识别槽若需 CI 可测，v3 的 mock / 自带 store trait 是可直接照抄的形态。

---

### 3.11 R7 watchkey-win（Windows Hello）

```
Windows Hello 签名（TPM 内，不可导出）──HKDF-SHA256──▶ AES key ──▶ 包裹本地密钥材料
```

（`references/4-vault-security/watchkey-win/src/crypto.rs`、`src/auth.rs`、`src/companion.rs`）

**实际做法**

- **challenge 是固定的，不是随机的**：`fixed_challenge() = SHA256("watchkey-auth-challenge-v1")`，所有认证请求都用同一串（`auth.rs:171-177`）。这依赖 Hello 签名对同一 challenge **确定性**输出——代码里存 `signature_hash`（签名的 SHA-256）正是用来事后校验这一点（`auth.rs:275-280`、`main.rs:73-116`）。
- **派生**：`HKDF-SHA256(salt = b"watchkey-v1", ikm = signature)` → 32B AES-256 密钥（`crypto.rs:10-17`）。Hello 私钥在 TPM 内不可导出。
- **包裹**：随机生成 32B `master_key`，用上述密钥做 AES-256-GCM，密文格式 `base64(nonce ‖ ciphertext)`，nonce 每次随机（`crypto.rs:19-56`）。
- **失效处理不静默**：解不开时先看 `signature_hash` 是否已存在——存在即说明签名变了（重装/换机/重置 PIN），直接返回 `MasterKeyCorrupted`，**不会用新签名重新包裹一把新密钥**（`main.rs:86-103`）。这是设计使然而非缺陷。
- **可选 companion（远程审批）**：`enable(master_key)` 才为当前用户额外生成**第二份 master-key 包裹**，用 Windows DPAPI（`DataProtectionProvider`，`LOCAL=user`）保护；解封需与 Companion 服务端**配对**（P-256 `EphemeralSecret` ECDH 协商会话，AES-256-GCM 且 `aad = request_id` 防串号），由远端审批，审批方式分 `passwordless` 与 `password`（后者走 `LogonUserW` 校验本机账户口令），`MAXIMUM_PROMPT_DURATION = 125s`、响应体上限 1 MiB。`disable()` 清掉这份包裹，`unpair()` / `reset()` 删配置文件。**注册本身不创建它，默认关闭**，模块头注释明确写「有意弱于 Hello 绑定」；`denied` / `expired` / `InvalidPassword` 即终态，不会静默回退到 Hello。它是**同一用户本机的备用包裹 + 远程审批**，不跨设备、不可迁移，因此不是恢复通道。
- **缺失项**：没有版本、没有轮换、没有多设备语义。

**定位**：硬件绑定、无口令、抗离线复制；**结构上正是「随机数据密钥 + 凭据派生密钥包裹」的形态**，与本项目 P2 的生物识别槽同构——差别只在它把包裹存在本机配置文件、没有版本号、也没有把 payload 交给 OS keystore 长期保管。但它只解决「本机解锁」，**签名变化（重装/换机）即失效**，不解决跨设备与恢复。本项目 v1 的生物识别槽与此同构；v1.5 的生物识别槽在此基础上多了「payload 可换代」与「payload 落 OS keystore 跨重启」两件事。

---

### 3.12 R8 LUKS / BitLocker（行业基线）

- **LUKS**：随机 Volume Key（VK）不在 header 里明文出现，而是**每个 keyslot 各存一份「被该 slot 的 KEK 加密后的 VK」**；KEK 由口令经 PBKDF2（LUKS1）或 Argon2i/id（LUKS2）派生，KDF 参数**每 slot 独立**。**改口令 = 只重写该 keyslot**（O(1)）；**真吊销 = `cryptsetup reencrypt`**（换 VK 并重加密全部数据，O(n)）。
- **BitLocker**：VMK 包 FVEK；VMK 可被多种「key protector」分别包裹——TPM、PIN、启动密钥文件、以及 **48 位恢复密码**（本质是另一把独立 KEK）。改 PIN 只换一个 protector。

**定位**：keyslot 模型原型。权限半径与吊销语义分得最清楚，但**没有 payload 裁剪概念**（要么全开、要么不开）；恢复凭据与主凭据**权限完全相同**。

---

## 4. 六维度横向对比

### 4.1 密钥层次：谁才是真正的数据密钥

| 模型 | 数据密钥是随机生成的吗 | 数据密钥与凭据解耦 | 层次数 | 改密复杂度 |
|---|---|---|---|---|
| P1 当前 v1 | ✗（口令派生） | ✗ | 3×Argon2id + HKDF | O(n) 全库重加密 |
| P2 目标 v1.5 | ✓（`dek` 随机；`dbKey` 由 `KEK` 派生） | ✓ | 1×Argon2id + HKDF | O(1) 重包 + 本机 `rekey` |
| R1 KeePassXC | ✗ | ✗ | 1×KDF | O(n) |
| R2 rust-keepass | ✗ | ✗ | AES-KDF 多轮 | O(n) |
| R3 Vaultwarden | ✓ | ✓ | 1×KDF | O(1) 重包 |
| R4/R5 prs/ripasso | 无 vault key | 不适用 | 0 | O(n) |
| R6a/b/c keyring 三件套 | 不适用（不提供数据密钥） | 交给 OS | 0 | 不适用 |
| R7 watchkey-win | ✓（32B 随机 `master_key`） | ✓（被 HKDF(Hello 签名) 派生密钥包裹） | 1×HKDF + AES-256-GCM | O(1) 重包（重新注册） |
| R8 LUKS | ✓ | ✓ | 1×KDF/slot | O(1) 重写 slot |

**判断**：「数据密钥是否随机」这一列直接决定其余四列。P1 与 R1/R2 同组（口令即密钥根），P2 与 R3/R7/R8 同组（随机密钥 + 包裹）。**R7 值得单独注意**：它虽然凭据是 Hello 签名，但**数据密钥是随机的、被签名派生密钥包裹**——因此它在结构上正是 P2 生物识别槽的同构原型，而不是「签名当密钥用」的反例。

### 4.2 凭据→密钥派生路径与离线猜测面

| 模型 | salt 来源 | salt 是否公开 | 是否每用户/每槽独立 | KDF | 解锁时 KDF 次数 |
|---|---|---|---|---|---|
| P1 当前 v1 | `BOOTSTRAP_SALT` **编译期常量** + 2 个随机 salt | 常量公开 + meta 内 salt 公开 | **BK 不独立（全用户相同）** | Argon2id 64M/t3/p4 | **3 次** |
| P2 目标 v1.5 | 每库随机（单 slot） | slot 内公开 | ✓ | Argon2id（参数随 slot） | **1 次** |
| R1 KeePassXC | header 内随机 | 公开 | ✓ | AES-KDF / Argon2（header 指定） | 1 次 |
| R2 rust-keepass | header 内 | 公开 | ✓ | AES-256-ECB 多轮（AES-KDF） | 1 次 |
| R3 Vaultwarden | 每用户随机 | 公开 | ✓ | PBKDF2 / Argon2id | 1 次 |
| R4/R5 prs/ripasso | — | — | — | GPG 自管 | — |
| R6a/b/c keyring 三件套 | 无（无 KDF） | — | — | 无（OS 凭据库） | 0 |
| R7 watchkey-win | 固定 challenge（`SHA256("watchkey-auth-challenge-v1")`） | 本地配置 | 每机 | HKDF-SHA256（salt `watchkey-v1`，输入为 Hello 签名） | 1 次 |
| R8 LUKS | 每 slot | header 公开 | ✓ | PBKDF2 / Argon2 | 1 次/slot |

**三个要点**：

1. **所有模型都「公开 salt」**（header/meta/服务端），因此离线字典攻击对所有模型都成立，差别只在**是否可批量预计算**。P1 的 `BOOTSTRAP_SALT` 常量让「同一个密码在所有用户上派生出同一把 BK」——攻击者预计算一次即可批量命中，这是 P1 独有的缺陷（R1/R3/R8 都是每用户独立 salt）。
2. **P1 的 3 次 Argon2id 不等于 3 倍安全**。三把密钥都源自同一口令，攻击者只需命中一次即可同时拿到 BK 与 MK；三次派生只是把**每次猜测的成本**提高，而不是把搜索空间变大。P2 降到 1 次是纯粹的体验收益（约 3s → 1s），不是安全退步。
3. **P1 的 meta 泄漏面比 R1 更大**：`.vault-meta` 公开 `master_key_salt` 与 `hash_salt`，攻击者拿到仓库即可对**单个用户**做离线猜测；`BOOTSTRAP_SALT` 又是常量，两层都可离线。

### 4.3 恢复通道权限半径

| 模型 | 恢复通道 | 能开库 | 能拿应用会话 | 能读审计日志 | 能改库内数据 | 可撤销 |
|---|---|---|---|---|---|---|
| P1 当前 v1 | 恢复密钥（包 `BK‖MK`） | **能** | **能** | **能** | **能** | **不能** |
| P2 目标 v1.5 | 纸面直存 `dek`（无 slot） | 不能 | 不能 | 不能 | 能（仅条目，`RecoverySession`） | 需 `rotate_dek` |
| R1 KeePassXC | 无 | — | — | — | — | — |
| R2 rust-keepass | 无 | — | — | — | — | — |
| R3 Vaultwarden | Emergency Access | 能 | 能 | 能 | 能 | 能（撤销授权） |
| R4/R5 prs/ripasso | 无 | — | — | — | — | — |
| R6a/b/c keyring 三件套 | 无（OS 账户即凭据） | — | — | — | — | 删凭据条目 |
| R7 watchkey-win | 无（companion 非恢复凭据） | — | — | — | — | — |
| R8 LUKS | keyslot / 恢复密码 | 能 | — | — | 能 | 重写 slot |
| R8 BitLocker | 恢复密码 | 能 | — | — | 能 | 换 VMK |

**判断**：P2 是**唯一把恢复通道裁剪到「只能拿数据」的模型**。R1 没有恢复通道（丢失即全丢），R3/R8 的恢复通道都是完整凭据。这个取舍不是行业惯例，是本项目对「恢复密钥该不该能开库」的回答。

代价必须写清：**P2 的恢复密钥救不回原库**——审计日志、生物识别注册、设置项、revision 全丢，只能搬走条目。

### 4.4 改密 vs 吊销语义

| 模型 | 改密动作 | 改密是否让旧凭据对**当前数据**失效 | 真吊销方式 | 用户是否需要理解两件事 |
|---|---|---|---|---|
| P1 当前 v1 | 重加密 + `PRAGMA rekey` | **是** | 改密本身 | 否 |
| P2 目标 v1.5 | 重包 password slot + 本机 `rekey` | **否** | `rotate_dek` | **是**（认知负担） |
| R1 KeePassXC | 重加密 | **是** | 改密本身 | 否 |
| R2 rust-keepass | 重加密 | **是** | 改密本身（或改轮数） | 否 |
| R3 Vaultwarden | 重包 `akey` | **否** | Rotate encryption key | 是 |
| R4 prs | 移除 recipient（`remove` 须显式 `--recrypt`） | **有条件**（不重加密则旧 recipient 仍可解） | 移除 + 重加密 | 否 |
| R5 ripasso | 换 recipient 后全量重加密 | **是** | 同左 | 否 |
| R6a/b/c keyring 三件套 | 无库口令（凭据 = OS 账户） | 不适用 | 删凭据条目 | 否 |
| R7 watchkey-win | 无库口令（凭据 = Hello 签名） | 不适用 | 重新注册（重包） | 否（但需自建 fingerprint 才可观测） |
| R8 LUKS | 重写 keyslot | **否** | `reencrypt` | 是 |
| R8 BitLocker | 重包 VMK | **否** | 换 VMK + 重加密 | 是 |

**判断**：这一列**没有对错，只有产品选择**。行业里「解耦派」（LUKS/BitLocker/Bitwarden/P2）都必须把吊销拆成独立动作；「耦合派」（KeePassXC/P1/prs）免费获得吊销，代价是改密 O(n)。

**P1 在这一列上确实优于 P2**——这是 P1 唯一结构性占优的地方，不应在重构中被无意识丢掉。

### 4.5 多凭据扩展性

| 模型 | 多凭据语义 | 新增凭据成本 | 支持「同一数据、多把钥匙」 |
|---|---|---|---|
| P1 当前 v1 | 三条独立路径，但共用同一份 `BK‖MK` | 复制密钥材料 | ✗（是同一把密钥的副本） |
| P2 目标 v1.5 | **析取**（口令可取 `dek`；纸面本身就是 `dek`；开库另需 `dbKey`） | 新增一个 slot（结构已兼容） | ✓ |
| R1 KeePassXC | **合取**（全部必须有） | 新增一个组件 | ✗ |
| R2 rust-keepass | **合取**（口令 + keyfile） | 新增一个组件 | ✗ |
| R3 Vaultwarden | 析取（password / emergency access） | 新增一条授权 | ✓ |
| R4 prs | 析取（多 recipient） | 加一个公钥 | ✓ |
| R6a/b/c keyring 三件套 | 单凭据（OS 账户） | 加一条 store 条目 | ✗ |
| R7 watchkey-win | 单凭据（Hello） | 重新注册 | ✗ |
| R8 LUKS | 析取（多 keyslot） | 新增一个 slot | ✓ |

**判断**：P2 的 slot 列表是**唯一能同时容纳「密码 / 恢复 / 生物识别 / 未来 YubiKey / 多密码」的结构**。P1 虽然有三条路径，但三条路径共用同一份密钥材料，因此**不是多凭据，而是同一凭据的三种包装**——这正是改密必须同时改写密码槽、清空恢复槽的根因；而生物识别槽没有被改密流程覆盖，于是变成静默过期。

### 4.6 失效模式与可观测性

| 模型 | 主要失效模式 | 是否静默 | 可观测性 |
|---|---|---|---|
| P1 当前 v1 | 改密清空恢复密钥；生物识别 payload 静默过期；160bit 无校验位导致抄写错误与错误凭据同码 | **是**（`clear_emergency_data` 只写日志） | 差：恢复密钥使用无审计 |
| P2 目标 v1.5 | 旧口令仍可解当前 `dek`（C-1）；改密崩溃后本机库失配；旧 `dek` 仍解历史数据 | 部分（UI 需显式告知；失配需修复流程） | 改善：`manifest`/`anchor` 检出删除与回滚；本地 JSONL + 解锁补录 |
| R1 KeePassXC | 凭据组件丢失 = 全丢 | 否 | 好：HMAC mismatch 明确区分 |
| R2 rust-keepass | 凭据组件丢失 = 全丢 | 否 | 好：`PassErr` 明确，但无法区分「口令错」与「keyfile 错」 |
| R3 Vaultwarden | 接管即身份丢失 | 否 | 好：邀请/确认/等待期 + 邮件 |
| R4/R5 prs/ripasso | recipient 撤销不彻底（需重加密） | 否 | 中：git 提交可溯源 |
| R6a/b/c keyring 三件套 | store 条目被删 / 账户迁移 / OS 重装 → 解不开 | **是**（上层无 fingerprint 时） | 差：需自建校验 |
| R7 watchkey-win | 签名变化（重装 / 换机 / 重置 PIN）→ 解不开 | **是**（只报 `MasterKeyCorrupted`；虽有 `signature_hash` 记录，但两条分支返回同一错误，**不与密文损坏区分**，也不提示用户去重注册） | 差：错误码单一 |
| R8 LUKS / BitLocker | keyslot 口令错 / header 损坏 / 恢复密码丢失 | 否 | 好：header 校验 + 按 keyslot 定位，但「哪个 slot 有效」需尝试 |

**判断**：P1 在「失效静默」上是最差的——三个失效都不报警。P2 的最大改进不在密码学强度，而在**把失效变成可见事件**。

---

## 5. 各方优缺点清单

### 5.1 P1 当前实现 v1

**优点**

1. 改密即吊销：旧密码对当前数据立即失效（行业里「耦合派」的标准行为，比 P2 强）。
2. 三层派生把「校验口令」与「解密数据」解耦，错误码可分级（`InvalidPassword` vs `Crypto`）。
3. 恢复密钥不落明文，且与密码不是明文依赖关系（形式上像 KEK 包裹）。
4. 生物识别复用同一份密钥材料，实现成本低（一处加密、一处解密）。

**缺点**

1. **结构性**：`master_key` 兼任「口令派生物」与「数据密钥」，改密必然换密钥，恢复材料必然作废——`clear_emergency_data` 是被迫的，不是选择。
2. **`BOOTSTRAP_SALT` 常量**：跨用户口令预计算可行，一次预计算批量命中。
3. **恢复密钥 = 影子主密码**：解出 `BK‖MK`，能开库、能拿会话、能读审计、能改库，且**使用不可审计**。
4. **无校验位**：160bit 恢复密钥抄错与真错同码，用户无法区分「我抄错了」和「密钥不对」。
5. **改密 O(n) 非原子**：中途失败会留下半迁移状态。
6. **不变量靠调用方**：改密清恢复密钥写在命令层（`entries.rs:189`），库层 API 单独调用不会清。
7. **生物识别静默过期**：payload 里是旧 `MK`，改密后 `sig_hash` 检测不到「密钥换代」。

### 5.2 P2 目标 v1.5 单槽 + 纸面即密钥

**优点**

1. 数据密钥随机 → 改密 O(1) 重包，条目零改动；本机库 `rekey` 只是单文件操作。
2. 恢复通道只拿 `dek`，`dbKey = HKDF(KEK)` 且不落文件 → **恢复密钥在任何机器上都无法开库**。注意这条收益的**分量**：纸面即 `dek`，持有纸面的人已经能解开全部条目，所以它只保护本机基础设施（审计、生物识别注册、`anchor`），不构成数据泄露防线。它的真实价值是「纸面不是一份能坐到本机把应用正常登录使用的完整凭据」。
3. 仓库里不再有「包裹好的数据钥匙」：`emergency` 字段整体消失，恢复通道与仓库解耦。
4. 每库独立 salt/参数 → 消除跨用户预计算；未来调参不影响存量 slot。
5. 解锁 1 次 Argon2id，GCM tag 即认证（省掉独立 `hash_salt` 与校验字段）。
6. 恢复纸面自带 CRC32C + 分组 + 归一化 → 抄错秒级定位，且不构成密钥预言机。
7. 恢复通道返回 `RecoverySession`（只持 `dek`）→ 权限裁剪是编译期事实，不是运行时约定。
8. 改密不再连带清空恢复密钥（修 C3）；`manifest` + 本机 `anchor` 补上删除与回滚检测。
9. **额外文件为零**：应用侧只有 `.vault-meta` + `security.db`，用户随身零文件。

**缺点**

1. **改密不再吊销旧密码**（唯一真正的安全退步），需用户主动 `rotate_dek`。
2. **纸面泄露 = 全部条目泄露**（纸面就是 `dek`，无需口令）。这是「零随身文件」的代价。
3. **纸面无法单独撤销**：换纸面必须 `rotate_dek`（O(n) 重加密）。
4. **恢复密钥救不回原库**：审计、生物识别注册、设置、revision 全丢。
5. 改密必须 `PRAGMA rekey` 本机库；崩溃窗口会导致本机库失配，需修复路径。
6. 旧库需一次性迁移，迁移后 v1 不可解锁（旧文件仅归档）。
7. 实现复杂度上升：AAD 规范、纸面编解码、`fingerprint`、manifest/anchor、严格解析器都要新增（但比双槽方案少一套槽合并规则）。

### 5.3 R1 KeePassXC

**优点**：多组件模型干净（口令/密钥文件/硬件 key 平等，按加入顺序拼接后 SHA256）；header 双校验（SHA256 + HMAC）能明确区分「凭据错」与「文件损坏」；KDF 参数随库走，可平滑升级；challenge-response 是一等公民，且 KDBX4 起参与 transform；QuickUnlock 提供了不落盘的本机会话解锁。

**缺点**：**合取模型无恢复通道**，任一组件丢失即全丢（密钥文件是必需组件，不是恢复凭据）；改密 = 全库重加密；没有数据密钥层，无法做 O(1) 改密；QuickUnlock 只活在进程内存，跨重启即失效，因此不提供「payload 换代」问题的任何参考。

### 5.4 R2 rust-keepass

**优点**：实现简单，能读历史 KDB v1 库；keyfile 支持 32B 原始 / 64B hex / 其余长度按 2048B 分块哈希三种形态。

**缺点**：AES-KDF（AES-256-ECB 多轮）、非内存硬、抗 GPU 差；只支持 v1；**证明「KDF 选错，层次对了也没用」**。

### 5.5 R3 Vaultwarden / Bitwarden

**优点**：教科书式解耦（`akey` 独立于 password，改密只重包）；Emergency Access 是独立凭据而非复制主密码，且有 **View / Takeover 两级权限** + 邀请/确认/等待期/通知的流程约束；salt 与 KDF 参数（类型/迭代/内存/并行度）每用户独立且可被组织策略强制；组织密钥按成员公钥分发，不需要共享对称密钥。

**缺点**：Emergency Access 的 takeover 是**身份接管**（能改密码、清 2FA、踢组织），权限半径比 P2 大一个量级；「只读 View」靠服务端授权实现，不是密钥结构保证；服务端可见 salt/iterations → 离线字典攻击仍成立；服务端备份里的旧 `key_encrypted` 仍可解开（撤销不彻底）；流程依赖服务端，单机自托管场景不可直接照搬。

### 5.6 R4 prs

**优点**：多 recipient 天然 OR 语义，无 KEK 层；撤销语义清晰——`add` 默认重加密（`--no-recrypt` 可跳）、`remove` 需显式 `--recrypt`、`housekeeping recrypt` 可全量兜底；公钥随仓库 `.public-keys/` 走，不依赖 keyserver；无私钥口令派生 → 无离线字典攻击面。

**缺点**：撤销必然 O(n)；依赖 GPG 密钥管理，用户负担重；只有根目录一个 `.gpg-id`，子目录分层尚未支持；没有「同一用户多设备」抽象；git 历史保留旧密文，撤销对历史数据不彻底。

### 5.7 R5 ripasso

**优点**：条目=文件，与 git 天然契合；提交签名可溯源；`reencrypt_all_password_entries` 把「recipient 变更 → 全量重写」做成显式动作，语义不含糊。

**缺点**：**无统一 vault key** → 无法做会话密钥缓存与 slot；撤销/轮换 = 全量重加密；git 历史保留旧密文，撤销对历史数据不彻底；依赖外部 gpg 与 gpg-agent。

### 5.8 R6a / R6b / R6c keyring 三件套

**优点**：无自研 KDF，边界交给 OS；跨平台统一 API，适合作为生物识别槽后端。core 把 store 选择挪到运行时（`set_default_store` / `unset_default_store`），并自带 mock 与文件持久化 sample 两个测试 store；R6b 用 `v1` feature 让旧调用方零改动；R6c 的 mock / 自带 store trait 让 CI 可注入错误。

**缺点**：无「口令 + 恢复」模型；不适合作为 vault 主密钥唯一来源（OS 失陷即全失，且不可跨设备恢复）。R6c 仍把 store 选择放在编译期（feature 决定），同代码在不同平台行为不同、测试只能编译期替换。三者在版本 / 轮换语义上都是空白，payload 生命周期必须由上层自管。

### 5.9 R7 watchkey-win

**优点**：硬件绑定（签名不可导出）、无口令、抗离线复制；**随机 `master_key` + HKDF(签名) 包裹**，是 P2 生物识别槽的同构原型（重包 O(1)）；可选 companion 提供远程审批下的备用解封路径（默认关、明确弱于 Hello）。

**缺点**：只解决本机解锁；签名变化即失效且只报 `MasterKeyCorrupted`，用户无从得知该重新注册；challenge 固定（靠 Hello 签名确定性），一旦该确定性在某台机器上不成立就整体失效；companion 不跨设备、不可迁移，且开启即降低整体强度，因此仍不解决恢复。

### 5.10 R8 LUKS / BitLocker

**优点**：keyslot 原型，权限半径与吊销语义分得最清楚；每 slot 独立 KDF 参数；`reencrypt` 明确承担「真吊销」。

**缺点**：无 payload 裁剪概念（全有或全无）；BitLocker 恢复密码是完整开库凭据（靠 AD/云托管审计兜底）。

---

## 6. 对本项目的判断

### 6.1 该学的与不该学的

| 结论 | 来源 | 理由 |
|---|---|---|
| **学**：数据密钥与密码解耦 | R3、R8 | P1 的结构性缺陷只有这一条解；P2（v1.5）已采纳 |
| **学**：每库独立 KDF 参数 | R8、R1 | 参数可演进且不影响存量；顺带修掉 `BOOTSTRAP_SALT` 常量 |
| **学**：封装头双重校验 | R1 | P1 的 HMAC 覆盖全结构已够；P2 需保证 `slots` / `manifest` 也在覆盖内 |
| **学**：恢复通道的流程约束 | R3 | 但本项目无服务端，只能用「本地审计 + 用后置位」替代等待期 |
| **学**：生物识别槽 = 随机密钥被签名派生密钥包裹 | R7 | 与本项目 `BiometricKeyStore` 同构；但必须补上它缺的版本号与指纹校验 |
| **不学**：固定 challenge | R7 | 依赖 Hello 签名确定性，一旦不成立则整体失效；本项目应存随机 nonce 或指纹 |
| **不学**：合取式多凭据 | R1 | 会彻底失去恢复通道 |
| **不学**：恢复=账户接管 | R3 | 权限半径过大，正是 P1 的病 |
| **不学**：全有或全无 payload | R8 | 无法表达「只给数据不给库」 |

### 6.2 P2 相对所有参考项目的独特性

1. **本地库钥匙不落任何文件**：`dbKey = HKDF(KEK)`，仓库里只装 `dek` 的封装，纸面只装 `dek` 本体。R3/R8 的恢复通道都是完整凭据，**没有参考实现可抄**。
2. **改密与吊销显式拆成两个产品动作**，且要求把「旧密码仍有效」写进文案。R3/R8 在工程上拆了，但没有把它变成用户必须理解的语义。
3. **面对 git 历史这一威胁面**。R1 是单文件、R3 是服务端权威，都不需要处理「旧封装头可被重放」；因此约束 C-1 是本项目特有，**不能靠照抄参考项目绕过**。

### 6.3 原 v2 方案的三处未覆盖点（已在新设计中补齐）

| # | 问题 | 证据 | 影响 | 新设计中的处理 |
|---|---|---|---|---|
| 1 | 删掉 `crypto` 字段后，「远端仓库是否属于另一把主密钥」的判据失效 | `src-tauri/crates/assistant-sync/src/sync_engine/mod.rs:290-315` 比较 `master_key_salt`；`to_crypto_meta` 在同文件 `:1124-1137` 生成该字段 | `VaultKeyMismatch` 与 `VaultMetaIntegrityFailed` 无法区分，用户会看到错误的诊断信息 | 新增 `vaultId` 作为仓库身份判据（设计 §2.3、§6.2） |
| 2 | 生物识别 payload 存 OS keystore（不进 meta），但 payload 必须随改密 / `rotate_dek` 更新 | `src-tauri/crates/assistant-security/src/security_db/biometric_ops.rs:237-300` 包裹的是 `BK‖MK`；v1.5 改为包裹 `dek‖dbKey` | 换代后生物识别**静默失效**，且无指纹校验则无法区分「密钥换代」与「Hello 凭据变更」 | 存 `fingerprint = SHA256(dek‖dbKey)`，不匹配返回 `BiometricReset`（设计 §3.6、§7.2） |
| 3 | `clear_emergency_data` 在命令层而非库层 | `src-tauri/src/cmd/vault/entries.rs:189`；`src-tauri/crates/assistant-security/src/security_db/rekey.rs` 内部不调用 | v1.5 的改密完全不碰恢复材料，忘记删这行会变成新 bug：重包成功但命令层仍去删一个已不存在的字段 | 改动清单显式要求删除该调用，并在库层加断言（设计 §3.2、§6.1） |

---

## 7. 证据索引

| 对象 | 文件 | 关键位置 |
|---|---|---|
| P1 密钥派生 | `src-tauri/crates/assistant-security/src/vault_crypto/mod.rs` | :31-79 |
| P1 硬编码 salt | `src-tauri/crates/assistant-security/src/security_db/mod.rs` | :22、:67、:122、:144-176 |
| P1 恢复密钥 | `src-tauri/crates/assistant-security/src/recovery_key/mod.rs` | :16-24、:33-77、:127-204、:210-234 |
| P1 改密 | `src-tauri/crates/assistant-security/src/security_db/rekey.rs` | :15、:80-176、:215 |
| P1 改密清恢复密钥 | `src-tauri/src/cmd/vault/entries.rs` | :173-200（:189） |
| P1 生物识别 | `src-tauri/crates/assistant-security/src/security_db/biometric_ops.rs` | :22-30、:237-300、:303-354 |
| P1 meta 结构 | `src-tauri/crates/assistant-sync/src/vault_file/types.rs` | :95-123 |
| P1 仓库身份判据 | `src-tauri/crates/assistant-sync/src/sync_engine/mod.rs` | :290-315、:945-965、:1124-1137 |
| R1 CompositeKey | `references/3-vault-sync/keepassxc/src/keys/CompositeKey.h` | :30-62 |
| R1 密钥文件格式 | `references/3-vault-sync/keepassxc/src/keys/FileKey.cpp` | 全文 |
| R1 QuickUnlock | `references/3-vault-sync/keepassxc/src/quickunlock/WindowsHello.cpp` | 全文 |
| R1 KDF 类型 | `references/3-vault-sync/keepassxc/src/format/KeePass2.h` | KDF_* 常量 |
| R1 KDBX4 封装 | `references/3-vault-sync/keepassxc/src/format/Kdbx4Writer.cpp` | :45-68 |
| R1 KeeShare | `references/3-vault-sync/keepassxc/src/keeshare/KeeShare.cpp`、`KeeShareSettings.h`、`ShareExport.cpp` | :82-110、:121-133、:119 |
| R2 KDB v1 | `references/3-vault-sync/rust-keepass/src/kpdb/crypter.rs` | :147-194（合成）、:245-340（keyfile）、:362-395（transform_key） |
| R3 用户密钥 | `references/3-vault-sync/vaultwarden/src/db/models/user.rs` | :44-52、:201-220 |
| R3 组织/成员包裹 | `references/3-vault-sync/vaultwarden/src/db/models/organization.rs` | :32-56（Organization / Membership 字段） |
| R3 管理员重置 | `references/3-vault-sync/vaultwarden/src/api/core/accounts.rs` | :815-830、:920-1015 |
| R3 紧急访问 | `references/3-vault-sync/vaultwarden/src/api/core/emergency_access.rs`、`src/db/models/emergency_access.rs` | :558-694、:744-773（timeout job）、View/Takeover 类型 |
| R4 GPG 收件人 | `references/3-vault-sync/prs/lib/src/crypto/store.rs`、`crypto/mod.rs`、`cli/src/cmd/matcher/recipients/{add,generate,remove}.rs` | :13-45、:100-180、`Proto::Gpg`、`no_recrypt` / `recrypt` |
| R5 pass/git | `references/4-vault-security/ripasso/crates/ripasso/src/crypto.rs`、`pass.rs` | :670-765（`reencrypt_all_password_entries`）、:280 签名校验 |
| R6a 运行时选 store | `references/4-vault-security/keyring-core/README.md` | 全文 |
| R6b v1 兼容门面 | `references/4-vault-security/keyring-rs/README.md` | 全文 |
| R6c 编译期 feature 选 store / mock | `references/4-vault-security/keyring-rs-v3/README.md` | 全文 |
| R7 Hello 派生 | `references/4-vault-security/watchkey-win/src/crypto.rs`、`src/auth.rs` | 全文 |
| R7 可选 DPAPI 备用包裹（远程审批） | `references/4-vault-security/watchkey-win/src/companion.rs` | 全文 |
| P2 设计 | `./I9-vault-key-hierarchy-design.md` | §0.4、§2.1-2.8、§3、§4、§5、§11 |
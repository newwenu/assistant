# I9 密码箱 密钥体系设计（v1.5：`dek` 解耦 + 纸面即密钥）

> 状态：设计文档（2026-10-28 修订）
> **实施状态：未开始**（从 §8 的 P0-1 起）
> 取代：本文早期版本（v2 keyslot + 本地/同步双槽方案，已废弃，理由见 [附录 B](#附录-b被否决的两个方案)）
> 关联：`I9-vault-password-model-comparison.md`（横向对比基线）；I9-vault-spec-1-foundation、I9-vault-spec-5-ssh-and-recovery（已归档，仅作历史参照）
> 范围：密钥派生、密钥层次、文件布局、改密/轮换/吊销语义、恢复通道权限半径。不含 UI 视觉、同步协议细节、条目字段加密格式。
> **已确认前提（2026-11-01）**：①**不做 v1 → v2 迁移**——项目未投入生产、无存量用户，旧库一律拒绝并提示重置（§5）；②**同步尚未启用**——本期不设计 `.vault-meta` 的跨设备导出格式（§6.2、§10 D11）。
> 路径约定：文中所有路径均为**仓库相对路径**。

---

## 0. 摘要

### 0.1 一句话

**把「口令」从「数据密钥」里摘出来。** 条目由随机 `dek` 保护；口令只负责把 `dek` 包裹起来，恢复密钥就是 `dek` 本体。

### 0.2 三条公理

| 编号 | 公理 | 直接后果 |
|---|---|---|
| **A1** | 数据密钥 `dek` 随机生成，与口令无派生关系 | 改密只重包，不动条目；改密不连带作废恢复密钥 |
| **A2** | 恢复密钥就是 `dek` 本体，不依赖仓库中任何封装结构 | 仓库里不再存在「被包裹的数据钥匙」；恢复通道不需要读 `.vault-meta` 的密钥字段 |
| **A3** | 本机库钥匙 `dbKey` 由 `KEK` 派生，不落任何文件 | 纸面（`dek`）在任何机器上都无法开库；应用侧不新增任何文件。**代价**：改密必须对本机库 `PRAGMA rekey`，且每台设备各一次（§3.5） |

三条公理互相支撑：A1 让「改密」变成廉价动作，A2 让「恢复」不再需要第二份封装，A3 让「恢复通道」的权限半径由密码学裁剪而非 UI 约束。

但三条公理的**价值不等价**，必须分开记账（分档定义见 §0.6）：A1 直接保护用户唯一在意的数据（条目）；A2 买的是「零随身文件」与恢复通道不依赖仓库结构；A3 买的是「拿到磁盘不等于拿到库钥匙」，代价是改密必须动本机库。**A3 不是零成本**，它的账记在 §3.2 的崩溃窗口与 §3.5 的多设备修复上。

### 0.3 相对当前实现（v1）的改动一览

| 维度 | v1（当前） | v1.5（本设计） |
|---|---|---|
| 条目密钥根 | `master_key` = Argon2id(口令) | 随机 `dek`（20 字节，160 bit） |
| 仓库中的密钥材料 | `masterKeySalt` + `hashSalt` + `emergency` | 一个 password slot（包裹 `dek`） |
| 改密成本 | O(n)：重加密全部条目 + `PRAGMA rekey` | O(1)：重包 20 字节 + `PRAGMA rekey`（本机页密钥） |
| 改密对恢复密钥 | **作废**（`clear_emergency_data`） | **不作废**（纸面就是 `dek`，与口令无关） |
| 恢复密钥能开库 | **能**（`recovery_key::unlock_with_recovery_key` 解出 `BK‖MK` 后可开库并做探针读；仅测试引用，命令层未调用） | **不能**（`dbKey` 不在纸面里，且不再提供该函数） |
| 仓库中的恢复封装 | 有（`emergency: {token, salt}`） | **无**（仓库里没有恢复相关密钥字段） |
| 口令校验 | 独立 `hash_salt` + 第三次 Argon2id | GCM tag（省掉一次派生与一个字段） |
| 跨用户预计算 | 可（硬编码 `BOOTSTRAP_SALT`） | 不可（每库随机 salt） |
| 全熵密钥上的 `kdf_iter` | 256000（无意义开销） | 1 |
| 抄错检测 | 无 | 分组 + CRC32C（秒级定位） |
| 恢复通道类型 | `bool` + 全局连接（`unlock_with_recovery_key`）；产品命令只做条目导出 | `RecoverySession`（只持 `dek`，编译期无法开库） |
| 额外文件 | 恢复包（用户自带） | **零**（用户随身零文件） |

### 0.4 相对早期 v2 双槽方案的收敛

早期 v2 方案引入了 `local-meta.json`、双槽（本地槽 / 同步槽）、2 代历史槽、`payloadFingerprint`。本设计把它收敛成单槽 + 单文件：

| 维度 | 早期 v2 | v1.5 |
|---|---|---|
| 应用侧文件数 | `.vault-meta` + `local-meta.json` | `.vault-meta` + `security.db` |
| 槽数组 | 本地槽 + 同步槽，需合并规则 | 单槽，无合并规则 |
| 历史槽 | 需保留 2 代 | 不需要（见 §3.5 修复路径） |
| 恢复密钥来源 | recovery slot 包裹 `dek` | 纸面直接承载 `dek`，无 slot |
| 改密是否碰本机库 | 是（重包本地槽 + rekey） | 是（rekey，但不重包任何槽） |
| 多设备改密 | 需人工修复历史槽 | 只需输入旧口令一次（§3.5） |

**为什么能收敛**：早期 v2 需要 `local-meta.json`，是因为它把 `dbKey` 当作「随机且需被包裹」的独立密钥。本设计改为 `dbKey = HKDF(KEK, …)`——派生即得，无物可包，文件自然消失。

### 0.5 本设计明确**不**解决什么

诚实边界，避免误判收益：

1. **仓库仍然是口令的离线爆破靶**。`salt` + 包裹后的 `dek` 都在 git 里，拿到仓库的人可以离线跑 Argon2id 猜口令。v1.5 只能提高单次猜测代价（Argon2id 参数、每库独立 salt），不能消除这个靶。**唯一能消除的是把密钥域移出仓库**（附录 C）。
2. **纸面泄露 = 全部条目泄露**。纸面就是 `dek`，不需要口令即可解开所有条目文件。这是「零随身文件」的代价。
3. **纸面无法单独撤销**。纸面与 `dek` 同体，换纸面必然换 `dek`，即 `rotate_dek`（O(n) 重加密）。
4. **改密不吊销旧口令**。旧口令 + 仓库历史仍可解出当前 `dek`。真吊销只有 `rotate_dek`。
5. **单凭据**。本设计只有一个 password slot，不预留 YubiKey / 多口令扩展位（需要时再加 slot，结构已兼容）。
6. **恢复通道只带得回条目，带不回本机基础设施**（审计、生物识别注册、设置、`revision`、`anchor`）。这不是本设计新引入的损失：v1 的 `unlock_with_recovery_key` 解出 `BK‖MK` 后仍要求 `security.db` 存在，否则返回 `NotInitialized`（`recovery_key/mod.rs:184-185`），**换机器时 v1 的恢复密钥同样开不了库**；命令层实际走的灾难路径 `vault_disaster_recovery_export` 也只导出条目明文。换言之，在「机器丢了」这个唯一需要恢复的场景里，v1 与 v1.5 能带回的东西**完全相同**。这些数据本身也只在本机有效（`security.db` 不进 git），且用户对其丢失无感（§0.6 T3）。
7. **不设防 OS 端点攻破**。本设计假设「用户会话与 OS 凭据可信」：同用户代码执行、内核 / 管理员权限、物理接触并攻破 OS 登录、攻破 TPM / keystore，**均不在威胁模型内**。理由：这些能力已足以读取解锁后的进程内存（`master_key` / `bootstrap_key` / `conn` 常驻 `Mutex`），攻破 keystore 不构成新增路径，边际风险为零。因此生物识别**不按「第二因素」定位**，而是「口令的等价替代凭据」。
   - **推论（必须写入用户可见文案）**：开启生物识别后，保险箱强度 = **min(主密码强度, OS 登录强度)**。Windows Hello 在多数设备上退化为 4~6 位 PIN。这是产品承诺问题，不是密码学问题。
   - **不属于本条的三件事**（它们不依赖任何攻击发生，属于本设计自身的语义 / 实现问题，见 §7.2 R8/R9 与 §9.2 I12）：`rotate_dek` 不吊销生物识别、生物识别**失败 / 取消**路径无审计、`set_key_unprotected` 的默认回落。

---

### 0.6 数据分层：用户真正在意的是什么

本文所有「收益 / 代价」判断都按这张表分档，避免把基础设施的得失误记成安全得失：

| 档 | 数据 | 归属 | 丢失后果 | 用户感知 |
|---|---|---|---|---|
| **T1** | 条目明文（口令、TOTP 种子、SSH 私钥、安全笔记） | 用户资产 | 灾难 | 直接崩溃 |
| **T2** | `dek` 的可恢复性、改密成本、随身文件数 | 可用性 / 体验 | 「拿纸时才发现纸已废」；改密产生 n 文件 commit | 强烈 |
| **T3** | 审计日志、生物识别注册、`anchor`、`revision`、设置 | 本机基础设施 | 重建基线、重新注册、重新勾选 | 无感或一次点击 |

两个推论贯穿全文：

1. **恢复通道只导出条目，恰好覆盖 T1 的全部**。丢掉的是 T3，不是「一半数据」。而且 T3 只在「本机库还在、但口令忘了」这一个窄场景里才有差别——真正需要恢复的「机器丢了」场景，v1 也带不回 T3（§0.5-6 的代码证据）。
2. **「纸面不能开库」的收益基本为零，不应作为卖点**。三条理由叠加：
   - 它只保护 T3（纸面即 `dek`，持有者已能解开全部条目 T1）；
   - T3 只在本机有效（`security.db` 不进 git，换机器就没了，谈不上「保护」）；
   - T3 在 v1 的灾难场景下同样不可恢复，故不存在「v1 有、v1.5 没有」的落差。

   剩下唯一的增量是「攻击者同时拿到纸面与本机磁盘时，读不到审计日志」——而审计日志只含事件类型与条目 ID（`security_db/schema.rs:29-34`），不含条目名。**这条增量只作为事实差异记录（§0.3、§7.1），不作为卖点**。它的真实定位是「权限最小化」的副产品（`RecoverySession` 拿不到 `conn()`）。

---

## 1. 问题诊断

### 1.1 当前仓库里躺着什么

`.vault-meta` v1 的结构（`src-tauri/crates/assistant-vault/src/vault_file/types.rs`）：

```jsonc
"crypto":    { "masterKeySalt", "hashSalt", "argon2Params", "defaultCipher" },
"emergency": { "token", "salt" },
"integrity": "HMAC-SHA256(master_key, 全字段)"
```

三样东西同时进入了 git 历史：

1. **口令的 salt** → 支持离线字典攻击，且与数据在同一个包里。
2. **`emergency` 封装头** → 一个用恢复密钥派生的 KEK 包裹 `BK‖MK` 的密文。
3. **以 `master_key` 为 key 的完整性 HMAC** → 反过来又是一条口令校验通道。

### 1.2 由此推出的三个结构性问题

**S1｜没有数据密钥层，口令与数据强耦合。**
`master_key = Argon2id(口令)` 同时是「口令派生物」和「条目密钥根」。于是改密必然换根，必然重加密全部条目（`rekey.rs` 的 O(n) 路径），必然作废一切从旧根派生的材料——包括恢复密钥。`clear_emergency_data` 是被迫的，不是产品选择。

**S2｜恢复密钥在库层是一份完整凭据。**
`recovery_key::unlock_with_recovery_key` 解出 `BK‖MK` 后直接调用 `set_conn_and_key`，返回 `bool`：能开库、能读审计、能改库、能导出全部条目，与主密码在权限上没有区别。该函数当前仅被测试引用、命令层未调用（产品侧恢复走 `vault_disaster_recovery_export`，只导出条目明文），但它是库的公开 API，能力就在那里。

**S3｜仓库是钥匙库，不是数据仓库。**
仓库里既有数据（`entries/*.enc`），又有开数据的钥匙（`crypto` 字段），还有第二把钥匙的锁（`emergency` 字段）。这使「仓库泄露」与「数据泄露」在威胁模型上不可分离。

### 1.3 约束 C-1（本项目特有，参考项目都没有）

> **C-1**：一旦 `.vault-meta` 进入 git 历史，任何「删除 / 覆盖旧封装头」的动作都**无法真正吊销**旧凭据。旧凭据可以从历史里取回，配合当前仓库解出当前数据密钥。

推论：

- 在「数据密钥不变」的前提下，**任何 AAD / 版本号 / 字段绑定都无法阻止整块旧 meta 的重放**——攻击者拿走旧 meta 整块文件即可，AAD 里的每个字段都是旧的、且自洽。
- 因此 **「改密」与「吊销」必须是两个不同的产品动作**：改密 = 重包（不吊销），吊销 = 换 `dek`（`rotate_dek`）。
- 唯一的缓解手段是**本机 anchor**（§2.8）：本机拒绝被回滚到旧状态；但跨机器与纯离线攻击者不受此约束。

### 1.4 本设计的选择

| 问题 | 选择 |
|---|---|
| S1 | 引入随机 `dek`，口令只做 KEK（A1） |
| S2 | 恢复纸面只承载 `dek`，`dbKey` 由 `KEK` 派生且不落文件（A2 + A3） |
| S3 | 仓库里只留「口令包裹 `dek`」这一个最小封装；恢复相关字段从 schema 中删除 |
| C-1 | 显式拆开「改密」与「吊销」，并把「旧口令仍有效」写进 UI 文案 |

---

## 2. 目标架构

### 2.1 密钥层次

```
                    ┌──────────────────────────────────────────┐
                    │  纸面（永不进 git，永不进本机）            │
                    │  dek  20B = 160 bit  →  40 字符           │
                    └──────────────────┬───────────────────────┘
                                       │ 本体即密钥
                                       ▼
  password ──Argon2id(salt_kdf)──▶ KEK ──AES-256-GCM──▶ wrappedDek ──▶ dek
                                    │                                    │
                                    │ HKDF("i9:v1:sqlcipher:"‖vaultId)    │ HKDF("i9:v1:entry:"‖entryId)
                                    ▼                                    ▼
                                  dbKey(32)                        entryKey(32)
                                    │                                    │
                                    ▼                                    ▼
                        SQLCipher 页密钥(security.db)          entries/*.enc + 库内加密列
                                                                         │
                                          HKDF("i9:v1:meta-integrity") ──┘
                                                         ▼
                                            .vault-meta 的 HMAC 密钥
```

关键点：

- `dek` **只被口令包裹一次**，不参与任何 KDF 之外的派生链上游，因此换口令不影响 `dek`。
- `dbKey` **只由 `KEK` 派生**，不落文件、不进仓库、不在纸面。它保证「拿到磁盘 ≠ 拿到库钥匙」，但也意味着**改密必然要 `PRAGMA rekey` 本机库，且每台设备各一次**（§3.5）。这是 A3 的完整代价，不是零成本；「派生即得，无物可包」只说明了它省掉了文件，没有说明它把成本移到了改密路径上。
- `entryKey` 与 `dbKey` 同源不同支：都从「数据侧」派生，但一条走 `dek`、一条走 `KEK`，互不可达。

### 2.2 数据位置

应用自己管理的文件只有两个（`security.db` 与 OS keystore 由系统托管，不是应用写的明文文件）：

| 位置 | 文件 | 进 git | 内容 |
|---|---|---|---|
| 仓库 | `.vault-meta` | 是 | `vaultId`、`dekVersion`、password slot、manifest、`recoveryBackup` 标记、integrity |
| 仓库 | `entries/*.enc` | 是 | 条目密文（`entryKey` 派生） |
| 本机 | `security.db` | 否 | 条目索引/缓存、审计日志、`anchor`、生物识别注册信息（不含密钥） |
| 本机 | OS keystore | 否 | 生物识别包裹后的 `dek‖dbKey` |

**被删除的东西**：`local-meta.json`、`emergency` 字段、`crypto.masterKeySalt`、`crypto.hashSalt`、恢复包（用户自带文件）。

用户视角：**随身零文件**。唯一的离机备份是那张（或那串）恢复密钥。

### 2.3 `.vault-meta` v2 schema

> 命名说明：本设计称「v1.5」指的是**密钥体系代际**；`.vault-meta` 文件的 `version` 字段值是 **2**（因为 v1 的 `crypto`/`emergency` 结构必须被硬切）。两者不要混用。

```jsonc
{
  "version": 2,
  "vaultId": "9f1c…",                       // 仓库身份判据，首次初始化随机生成，永不变
  "dekVersion": 3,                           // 每次 rotate_dek +1
  "cipher": "aes256Gcm",
  "slots": [
    {
      "id": "pwd-3",                         // 单调命名，便于日志与诊断
      "type": "password",
      "payloadKind": "dek",
      "kdf": { "algo": "argon2id", "salt": "b64(16)", "m": 65536, "t": 3, "p": 4, "v": 19 },
      "wrappedDek": "b64(nonce(12) ‖ ct(20) ‖ tag(16))",
      "createdAt": 1761600000000
    }
  ],
  "manifest": {
    "count": 128,
    "entries": [ { "id": "e-01", "sha256": "b64", "dekVersion": 3 } ]
  },
  "recoveryBackup": { "dekVersion": 3, "confirmedAt": 1761600000000 },
  "integrity": "b64(HMAC-SHA256(metaKey, 除本字段外全部字段的规范化 JSON))"
}
```

字段规则：

- `slots` 是数组但**当前只有一个元素**。保留数组是为了将来加 slot（YubiKey、第二口令）时不需要 schema 变更。
- `payloadKind` 目前只有 `"dek"`。它的存在是为了让未来出现 `"dbKey"` 之类的 payload 时，AAD 能阻止 kind 降级/互换。
- `wrappedDek` 明文长度固定 20 字节；总长 48 字节，Base64 后 64 字符。
- `recoveryBackup` **不含任何密钥**，只记录「用户确认已备份 `dekVersion=N` 的纸面」。它被 integrity 覆盖，因此篡改可见。
- 解析必须**严格**：`version` 缺失、`slots` 为空、`payloadKind` 未知、`kdf.algo` 未知 → 一律返回 `Unknown`，**禁止任何静默降级**。当前代码两处必须改：`read_vault_meta` 完全不校验 `version`（`vault_file/mod.rs:260-263`，直接反序列化）；`write_emergency_data` 用 `.unwrap_or(VAULT_META_VERSION)` 静默沿用旧版本号（`:327-331`）。
- `version = 1`（v1 的 `crypto` / `emergency` 结构）→ `UnsupportedVaultVersion`。**不做兼容解析、不做字段映射**，处置见 §5。

### 2.4 派生规则

统一使用 HKDF-SHA256（`salt = 空`，`info` 见下），输出 32 字节：

| 用途 | `info` | 输入密钥材料 |
|---|---|---|
| 条目密钥 | `i9:v1:entry:` + `entryId` | `dek` |
| meta 完整性密钥 | `i9:v1:meta-integrity` | `dek` |
| 本机库页密钥 | `i9:v1:sqlcipher:` + `vaultId` | `KEK` |

- `KEK` 直接从 Argon2id 取 32 字节，不经 HKDF。
- 条目密钥**按条目派生**，不做「一把钥匙解全部条目」。同一 `entryId` 在 `entries/*.enc` 与库内加密列使用同一把，保证两处密文可交叉校验。
- `info` 前缀带 `i9:v1:` 版本，为将来算法迁移留出无歧义命名空间。**此处的 `v1` 指密钥层次代际，与 `.vault-meta` 的 `version: 2` 无关**（§2.3 命名说明），实现时照抄 `i9:v1:` 字面量，不要「顺手改成 v2」。

### 2.5 AAD 规范

GCM 的 AAD 固定为：

```
"i9:v1:slot" ‖ 0x00 ‖ payloadKind ‖ 0x00 ‖ slotType ‖ 0x00 ‖ slotId ‖ 0x00 ‖ be64(dekVersion)
```

作用：

- 阻止**槽互换**（把 biometric 的包裹塞进 password 槽）。
- 阻止**kind 降级**（把 `dek` payload 当 `dbKey` 用）。
- 阻止**跨版本混用**（`rotate_dek` 后的旧槽塞进新 meta —— 虽然旧 `dek` 本身已不可用，但显式失败比「解出错误数据」好）。

**AAD 的能力边界（必须写进注释）**：当 `dek` 未变（仅改密）时，旧 meta 的 AAD 与新 meta 完全一致，整块旧 meta 重放**无法被 AAD 拦截**。这是 C-1 的直接体现，不要指望 AAD 解决它。

### 2.6 解锁顺序（口令路径）

```
1. 读 .vault-meta
   ├─ 不存在 ──▶ NotInitialized
   ├─ version = 1，或 version 缺失但存在 `crypto.masterKeySalt`（v1 结构）──▶ UnsupportedVaultVersion（§5：不迁移，仅提示重置）
   ├─ version 非 2 / 无法解析 ──▶ Corrupted
   └─ 取 slots[0]，校验 type/payloadKind
2. KEK = Argon2id(口令, slot.kdf)                     ← 全流程唯一一次 Argon2id
3. dek = AES-256-GCM.Open(KEK, wrappedDek, AAD)
   └─ 失败 ──▶ InvalidPassword                        ← 口令错与数据损坏在此不可区分，属正常取舍
4. metaKey = HKDF(dek, "i9:v1:meta-integrity")
   HMAC 校验 integrity
   └─ 失败 ──▶ VaultMetaIntegrityFailed               ← 此处已证明口令正确，故是篡改而非口令错
5. dbKey = HKDF(KEK, "i9:v1:sqlcipher:" ‖ vaultId)
   PRAGMA key = x'…'; 打开 security.db
   └─ 打不开 ──▶ LocalDbKeyMismatch（§3.5 重建）
6. 读本机 anchor，比对 { metaHash, dekVersion }
   └─ 不匹配 ──▶ VaultRollbackDetected（需用户显式确认）
7. （pull 时）比对 manifest 与 entries/*.enc 实际 sha256
   └─ 不匹配 ──▶ VaultManifestMismatch
```

设计意图：

- **第 3 步与第 4 步的顺序不可交换**。先 GCM（证明「这把 `dek` 是用这个口令包出来的」），再 HMAC（证明「这份 meta 是被这把 `dek` 签过的」）。这样错误码才有意义：`InvalidPassword` 与 `VaultMetaIntegrityFailed` 是两件不同的事。
- **口令正确性只靠 GCM tag**，不再需要 `hash_salt` 与第三次 Argon2id。省掉一个字段、一次派生、一条独立的口令校验通道。
- `Argon2id` 参数每个库独立随机 salt，消除 `BOOTSTRAP_SALT` 带来的跨用户预计算。

### 2.7 恢复路径（纸面路径）

```
1. 归一化 + 解码纸面字符串（§4.1）
   └─ CRC 不符 ──▶ InvalidRecoveryKeyChecksum（"请检查第 3 组第 2 位"）
      └─ 定位方式：40 位置 × 31 替换值逐个重算 CRC，唯一通过者即错位（本质是纠错，安全含义见 §4.4）
2. dek = 解码结果（20 字节）
3. 读 .vault-meta，取 manifest
4. metaKey = HKDF(dek, "i9:v1:meta-integrity")，校验 integrity
   └─ 失败 ──▶ InvalidRecoveryKeyOrMetaTampered   ← 纸面抄错（CRC 侥幸通过）与 meta 被篡改在此不可区分
5. 逐条：entryKey = HKDF(dek, "entry:"‖id)，解密 entries/*.enc，校验 sha256 与 manifest 一致
6. 输出为 RecoverySession（只读、只能导出，见 §4.5）
```

**这条路径完全不触碰 `security.db`**：不需要 `dbKey`，不需要 `KEK`，不需要 Argon2id。因此它天然无法开库、无法读审计、无法改库——不是靠 UI 拦住，而是**在密钥层面没有可用的钥匙**。

第 4 步的错误不可拆分：解码后的 `dek` 恒为 20 字节（格式固定），所以「长度不一致」不构成区分依据。CRC 已通过却 HMAC 失败，只可能是「纸面抄错但 CRC 侥幸碰撞」或「meta 被篡改」，两者在密码学上不可区分——与 §2.6 第 3 步「口令错与数据损坏不可区分」是同一种取舍，统一返回一个错误码并在 UI 上同时提示两种可能。

### 2.8 完整性与回滚

三层：

| 层 | 机制 | 防什么 | 存哪 |
|---|---|---|---|
| L1 | `integrity` = HMAC(`metaKey`, 全字段) | 篡改 `slots`/`manifest`/`recoveryBackup` | `.vault-meta` |
| L2 | `manifest` = 每条目 `sha256` + `dekVersion` | 删除条目、篡改条目、**整块旧 meta 重放**（旧 manifest 与当前条目不符） | `.vault-meta` |
| L3 | `anchor` = { `metaHash`, `dekVersion`, 单调递增 `revision` } | 本机被回滚到旧状态 | `security.db` 的 `vault_meta` 表 |

- L2 对 C-1 有部分缓解：重放旧 meta 会让「旧 manifest」与「当前条目集合」对不上，从而被发现。但它**不能**吊销旧口令——旧 meta + 旧口令仍能解出当前 `dek`（因为 `dek` 未变），只是随后会在 manifest 校验处暴露。这是「检测」而非「阻止」。
- L3 只在**本机**有效。攻击者直接持有仓库（不走本应用）时，anchor 不参与。
- L3 在 `security.db` 被重建后失效一次（重建会重建基线）。这一点必须在文档与日志里写清楚。

---

## 3. 操作语义

### 3.1 操作矩阵

| 操作 | 动 `dek` | 动 `dbKey` | 动条目 | 动 `.vault-meta` | 动 `security.db` | 动纸面 |
|---|---|---|---|---|---|---|
| 解锁 | — | — | — | 读 | 读 | — |
| 改密 | 否 | **是** | 否 | 重包 slot | `PRAGMA rekey` | 仍有效 |
| `rotate_dek` | **是** | 否 | **全部重加密** | 新 slot + 新 manifest + `dekVersion+1` | 重加密列 | **需重抄** |
| 备份纸面 | 否 | 否 | 否 | 写 `recoveryBackup` | 否 | 生成/展示 |
| 生物识别注册 | 否 | 否 | 否 | 否 | 写注册信息 | 否 |
| 撤销生物识别 | 否 | 否 | 否 | 否 | 删注册信息 | 否 |

### 3.2 改密

```
输入：旧口令、新口令
1. KEK_old = Argon2id(旧口令, slot.kdf)；dek = Open(KEK_old, wrappedDek)
   └─ 失败 ──▶ InvalidPassword
2. 新 salt；KEK_new = Argon2id(新口令, salt_new)
3. dbKey_new = HKDF(KEK_new, "i9:v1:sqlcipher:" ‖ vaultId)
4. 备份 security.db → 执行 PRAGMA rekey = x'dbKey_new'
5. wrappedDek' = Seal(KEK_new, dek, AAD(slotId', dekVersion))
6. 原子写 .vault-meta（临时文件 + rename）  ← 提交点
7. 更新本机 anchor 的 metaHash；重包生物识别 payload（§3.6）
8. git commit
```

要点：

- **不重加密任何条目**。这是 A1 的全部价值：改密从 O(n) 降到 O(1)（`PRAGMA rekey` 是 O(DB 页数)，但那是本机单文件操作，不产生 n 个文件 commit）。
- **不作废纸面**。纸面是 `dek`，与口令无关。
- **`slot.id` 递增**（`pwd-3` → `pwd-4`），便于日志区分「哪一代封装」。
- **崩溃窗口**：第 4 步与第 6 步之间中断，`security.db` 已是新钥匙、`.vault-meta` 仍是旧 slot。后果：旧口令能解出 `dek`（meta 旧），但打不开库。修复路径见 §3.5（重建缓存），不需要任何额外文件。
- `PRAGMA rekey` 中断可能损坏 `security.db`。它只是**可重建的缓存**（条目真身在 `entries/*.enc`，审计真身在 JSONL），因此损坏的代价是「重建 + 从 JSONL 回放审计」，不是数据丢失。
- **禁止**在命令层调用 `clear_emergency_data`（当前 `src-tauri/src/cmd/vault/entries.rs:259`）。改密与恢复材料彻底无关。

### 3.3 `rotate_dek`（真吊销）

```
1. dek_new = 20 字节 CSPRNG
2. 全部条目：用 entryKey_old 解密 → entryKey_new 重加密 → 原子替换
3. 库内加密列：同样重加密
4. slots[0] = 用当前 KEK 包裹 dek_new；dekVersion + 1
5. manifest 全量重算；recoveryBackup 重置为空（纸面已失效）
6. 原子写 .vault-meta；git commit
7. 提示用户：旧纸面已失效，请立即抄写新纸面
```

要点：

- `dbKey` **不变**（它由 `KEK` 派生，与 `dek` 无关）→ **不需要 `PRAGMA rekey`**。
- 这是唯一能吊销旧口令 / 旧纸面 / 泄露的历史封装的操作，且必然 O(n)。
- `dekVersion` 单调递增，写进 AAD 与 manifest，防止新旧混用。
- 生物识别 payload 里的 `dek` 需重包（§3.6），否则静默失效。

### 3.4 恢复纸面的语义

| 动作 | 命令 | 说明 |
|---|---|---|
| 备份/查看 | `vault_export_recovery_key` | 解锁状态下展示 40 字符；写 `recoveryBackup` 标记 |
| 使用 | `vault_recover_from_paper` | 只读恢复，返回 `RecoverySession` |
| 撤销 | `vault_rotate_dek` | **没有独立的撤销操作** |

关键认知：**纸面不是「一把独立的钥匙」，它就是 `dek` 的编码形式**。因此：

- 「重新生成恢复密钥」= `rotate_dek`，不是「换一张纸」。UI 文案必须这么说，否则用户会以为换纸很便宜。
- 纸面泄露 = `dek` 泄露 = 全部条目泄露，且**不需要口令**。生成纸面时必须强警告（见 §4.6）。
- 纸面**永远无法开库**，因此它可以放心地写在纸上、拍在手机里、贴在保险柜里。

### 3.5 本机库失配（`LocalDbKeyMismatch`）

触发场景：

1. 本机改密崩溃（§3.2 第 4~6 步之间）。
2. 在设备 B 改了口令，设备 A 用新口令解锁。
3. 手工恢复了旧的 `security.db` 备份。

**v1 旧库不属于本节**：`.vault-meta` 为 `version=1` 时在 §2.6 第 1 步即被拒绝（`UnsupportedVaultVersion`），不会走到 `LocalDbKeyMismatch`。本节只处理「同一 `vaultId`、同一密钥体系下本机库与当前口令失配」。

统一修复路径（**无额外文件、无历史槽**）：

```
A. 提示："本机数据库使用了另一个口令派生的密钥"
B. 选项 1：输入旧口令 → 派生 dbKey_old → 打开库 → PRAGMA rekey 到 dbKey_new
   选项 2：重建本机缓存（从 entries/*.enc + JSONL 审计），丢弃旧库
C. 修复后更新 anchor 基线
```

选项 1 之所以可行，是因为旧 `KEK` 可从旧口令重算——**不需要保存任何历史槽或历史密钥**。选项 2 保证用户永远不会因为「忘了旧口令」而彻底卡死。

**成本必须写清**：`dbKey = HKDF(KEK, vaultId)` 意味着改密后**所有设备**的本机库都换了钥匙。改密的那台当场 `rekey`；其余设备下次用新口令解锁时**全部**落到 `LocalDbKeyMismatch`，需要各自修复（选项 1 或 2）。**N 台设备 = N 次修复**，且每次都要用户在场输入旧口令或确认重建。

这是单槽方案的直接代价（附录 B.1「代价」段只写了「改密必须 rekey」，未写「每台设备各一次」，此处补齐）。**但要说准口径**：相对 v1 这**不是新增成本**——v1 的 `dbKey` 同样随口令变，多设备改密本来也要各修一次；相对早期 v2 双槽方案它才是新增的（那版 `dbKey` 与口令无关）。它不是实现细节，也不能靠「历史槽」消掉——只要 `dbKey` 由 `KEK` 派生，这个乘法就存在。若将来这个成本不可接受，唯一的解法是让 `dbKey` 独立于 `KEK`（即重新引入一个本机文件），也就是否决 B.1 的理由反向成立。

### 3.6 生物识别

payload 结构与流程：

```
payload     = dek(20) ‖ dbKey(32)                  ← 52 字节
fingerprint = SHA256(payload)                      ← 与 payload 一起存 OS keystore 元数据
外层        = OS keystore（Windows Hello / DPAPI）包裹
```

- 生物识别解锁 = 取回 `dek‖dbKey`，跳过口令与 Argon2id。因此 `dbKey` 必须随 payload 走。
- **`fingerprint` 的作用**：改密 / `rotate_dek` 后，若重包失败（例如 OS 凭据需用户在场而用户取消），payload 变成过期值。下次生物识别解锁时，用口令路径已得到的 `{dek, dbKey}` 计算指纹比对，不匹配 → `BiometricReset`，提示重新注册。没有指纹时无法区分「密钥换代」与「Hello 凭据变更」，用户会看到错误诊断。
- **重包时机**：改密（`dbKey` 变）与 `rotate_dek`（`dek` 变）都必须重包。重包需要 OS 凭据在场，故 UI 应合并成一次确认：「改密将同时更新生物识别凭据」。
- 重包失败不阻断改密主流程，只把生物识别标记为待重新注册。

---

## 4. 恢复密钥规范

### 4.1 格式

```
字符集：Crockford Base32（0-9 + A-Z，去掉 I / L / O / U）
长度：40 字符 = 5 组 × 8 字符
字节：25 = 1（版本 0x02）+ 20（dek）+ 4（CRC32C）
```

分组示例（`-` 仅用于展示，解析时忽略）：

```
K7M2QXF9-3TBAWZE5-RNCJ4YD6-PHK2SVG9-MTB7XQFD
└──── 8 ────┘ × 5 组
```

输入归一化（必须实现，否则用户抄写必失败）：

| 输入 | 归一化 |
|---|---|
| 小写 | 转大写 |
| `-`、空格、制表、换行 | 删除 |
| 全角字符（０-９、Ａ-Ｚ） | 转半角 |
| `O` / `o` | → `0` |
| `I` / `i` / `l` / `L` | → `1` |
| `U` / `u` | → `V`（Crockford 标准做法） |

### 4.2 为什么是 160 bit

`dek` 长度取 20 字节（160 bit）而非 32 字节（256 bit），是**为纸面可抄写性付的费**：

- 40 字符 = 5 组，正好落在「一行写完、一次抬头扫读」的范围内；60 字符（7.5 组，256 bit + 校验）会明显增加抄错率。
- 160 bit 对**随机生成**的密钥是无条件安全的：暴力搜索 2^160 不可行，且不涉及口令字典，无需 Argon2id 加固。
- 与「口令」形成明确对比：口令是低熵、需要 KDF 加固的；`dek` 是全熵、不需要 KDF 的。这也是为什么 v1.5 里 SQLCipher 内部 `kdf_iter = 1`（对全熵密钥的 KDF 迭代无意义）。
- 代价：`entryKey = HKDF(dek, …)` 的有效熵上限为 160 bit。对 AES-256 而言这不是弱点（160 bit 密钥搜索不可行），但必须在文档里说清楚，不要假装是 256 bit。

### 4.3 为什么不用助记词

| 方案 | 40 字符 Base32 | BIP39 词表（12 词） |
|---|---|---|
| 信息量 | 160 bit | 128 bit |
| 抄写长度 | 40 字符 | 12 词 ≈ 70 字符 |
| 抄错后能否定位 | **能**（按位重算 CRC，定位到具体字符） | 不能（校验和只报「整句有错」，不指出哪个词） |
| 歧义来源 | 需归一化（O/0、I/1） | 词形相似（`accept`/`except`） |
| 本地化 | 无语言依赖 | 需中文词表，跨语言不一致 |
| 实现成本 | 一个编解码器 | 词表 + 校验和 + 本地化 |

结论：Base32 + CRC 在「短、**可定位**、无语言依赖」三项上都更好，且不需要维护词表。助记词的价值在「人脑记忆」，而恢复密钥的使用场景是**从纸上抄进输入框**，不需要记忆。

### 4.4 「可验证性」与「预言机」是同一件事

用户曾提出「用哈希逆向难度实现恢复密钥的可验证性」。这个想法值得正面回应：

> **任何确定性校验值，本质上都是「给定候选值，判断是否通过」的预言机。** 真正的分水岭是两件事：它判断的是「**抄对了**」还是「**猜对了**」，以及单次猜测的代价。

三种做法的对比：

| 做法 | 单次猜测代价 | 是否值得 |
|---|---|---|
| 无校验，直接 GCM 试解条目 | 1 次 AES-GCM + 1 次 HKDF | 基准 |
| 存 `SHA256(dek)` 到 meta 做校验 | 1 次 SHA256 | **不可接受**：把 160 bit 搜索变成廉价的哈希搜索，且给攻击者一条无副作用的确认通道 |
| 存 `Argon2id(dek)` 做校验 | 1 次 Argon2id | 可接受但**无必要**：用户抄错时已能通过 CRC 定位，不需要密钥级校验 |

**本设计的选择：只在纸面上放 CRC32C（防抄错），不在任何存储介质上放 `dek` 的哈希。**

- CRC32C 是**线性、可伪造**的校验：给定任意 `dek`，攻击者都能算出配套 CRC 并构造出「校验通过」的字符串，因此它**不能**证明密钥正确。反过来说，对一个给定的 CRC 值，通过校验的 `dek` 仍有 2^160 / 2^32 = **2^128** 个，依旧不可穷举。它唯一的用途是「抄错必然被发现」（漏检概率 2^-32）。
- 而 `SHA256(dek)` 一旦落到仓库里，就等价于公开一个 2^160 的哈希靶，且**每次猜测只需要一次 SHA256**。这是净损失。

一句话：**恢复密钥不需要「可验证」，它需要「可抄对」。** 前者是危险的，后者是廉价的。

**两个必须分清的对象（常见误读）**：

| 问题 | 答案 | 依据 |
|---|---|---|
| CRC 校验的是口令吗 | **不是**，校验的是纸面上的 `dek` | CRC 只覆盖 25 字节（版本 + `dek` + CRC），与口令无关 |
| 仓库里存了口令吗 | **没有**，只有 `slot.kdf.salt` 与 `wrappedDek` | 口令从不落盘；无任何口令派生材料带校验值 |
| 仓库能否「知道」口令 | **不能**，但能**验证猜测**（离线 Argon2id + GCM 试解 slot） | 任何「口令包裹密钥」方案固有，与 CRC 无关；见 §0.5-1 与附录 C G1 |
| CRC 能否加速口令爆破 | **不能**，单次猜测仍是 1 次 Argon2id，CRC 不参与 | 两条链路互不连通 |

CRC 的真实代价是另一件事：**它是 `dek` 候选的廉价验证通道**（1 次 CRC32C 即可判断候选是否「抄对了」）。可利用的前提是攻击者已持有绝大部分 `dek`（如照片糊了 3 位），此时穷举 31³ ≈ 3 万次即可还原——即「纸面部分泄露 → 全文泄露」的放大器。**但它只放大已泄露的纸面，不触及口令**；且去掉 CRC 后同样的部分泄露仍可用 HKDF + HMAC over meta 验证（3 万候选仍是秒级），增量可忽略。另外 **CRC 只存在于纸面，不在仓库里**：只拿到仓库的攻击者手上没有这个校验值，无从使用。

### 4.5 `RecoverySession` 类型隔离

```rust
pub struct RecoverySession {
    dek: Dek,            // 20B 新 newtype；仅此一个字段
}
impl RecoverySession {
    pub fn export_all(&self) -> Result<Vec<PlainEntry>, VaultError>;
    pub fn list_ids(&self) -> Result<Vec<String>, VaultError>;
    // 没有 conn()、没有 db()、没有 exec()、没有 audit()
}
```

> `Dek` 必须是独立 newtype，不能复用 `MasterKey`：后者固定 `[u8; 32]`（`assistant-crypto/src/vault_crypto/types.rs:108`），而 `dek` 是 20B（D3）。

- 恢复路径的返回类型**不是** `bool`，而是只持 `dek` 的会话对象。从类型层面保证：恢复通道拿不到数据库连接，也就无法读审计、无法改库、无法注册生物识别。
- 这是把「权限裁剪」从**运行时约定**（当前 `unlock_with_recovery_key` 直接 `set_conn_and_key`，仅测试引用）改成**编译期事实**。参考项目中没有一个这么做（LUKS 的 keyfile、Bitwarden 的 Emergency Access 都是完整凭据）。
- 同时删除 `unlock_with_recovery_key`：**恢复密钥不存在「解锁」语义**，只有「导出」语义。

### 4.6 传递通道

| 通道 | 支持 | 说明 |
|---|---|---|
| 屏幕展示 + 抄写 | 是 | 默认路径；5 组分组显示，便于逐组抄 |
| 打印 | 是 | 一页纸，含分组框与校验说明 |
| 复制到剪贴板 | 谨慎 | 允许，但**必须**提示「剪贴板会被其他应用读取」，并 30 秒后清空（若平台允许） |
| 存成文件 | 谨慎 | 允许，但明确「这是明文密钥，等同于全部数据」 |
| 二维码 | 是 | 适合转存到离线设备；需标注「扫码即得全部数据」 |
| 邮件 / 云笔记 / 截图分享 | 禁止 | 文案明确劝阻 |

**必须呈现的强警告**（因为 A2 把「纸面 = 数据」这件事变得直白）：

> 这张纸就是你的全部数据。任何人拿到它，不需要密码就能读出你所有的条目。请把它当作保险柜钥匙，而不是备用密码。

### 4.7 无远端时的纸面失效

**本期同步未启用**，因此所有用户的仓库都只在本机（本地 git）。这意味着纸面在**本机磁盘损坏**时无用：条目文件本身没了，`dek` 再正确也没有密文可解。

UI 必须把这一点说清：「备份恢复密钥」旁边固定提示「恢复密钥只能解开条目，不能重建不存在的仓库；请同时备份仓库目录」。这不是边角情况，而是当前唯一场景——**纸面 + 仓库备份是两件必须同时做的事**，缺任一件都不构成恢复能力。启用同步后（§10 D11）这条提示才可降级为「仅本机模式」时出现。

---

## 5. 旧库处置（v1 数据不迁移）

原则：**不做迁移、不做兼容解析、不做归档。**

项目尚未投入生产，无存量用户，因此 v1 → v2 迁移的全部成本（staging 目录、`journal.json`、断点续跑、崩溃注入、旧文件归档、回滚能力）**不产生任何收益**，只会把 v1 的死代码与攻击面继续留在仓库里。

```
1. 检测：.vault-meta 的 version = 1（或缺失 version 但存在 crypto.masterKeySalt）
   ──▶ UnsupportedVaultVersion（明确错误，不是 LegacyVaultDetected）
2. UI：提示「此库由旧版本创建，本版本不支持升级」，并提供「重置仓库」入口
3. 重置仓库：二次确认 → 删除 .vault-meta / security.db / entries/ → 重新初始化
4. 不解析旧库任何字段（判代际只看 version / 特征键是否存在），不重加密，不产生任何中间文件
```

要点：

- **不静默删除**。即使没有线上用户，开发机上的库可能含真实凭据；检测到 v1 库时只报错，删除必须由用户显式确认。
- **不做 `.vault-legacy/` 归档**。归档会把旧封装头（`salt` + `emergency`）原样留在仓库里，等于把 v1 的攻击面（§1.1）永久保留。
- **不做 v1 `integrity` HMAC 校验**。校验旧签名需要保留整套 v1 解析逻辑，而它的规范化编码问题正是 §2.8 要重写的部分——留着它等于同时维护两套 HMAC 规则。
- 由此连带删除：`BOOTSTRAP_SALT`、`argon2Params` 读取、`crypto` / `emergency` 反序列化、`LegacyVaultDetected` 分支。
- 开发机升级时：先备份再手动删除旧库（见 §9.3 手工验收）。**半成品迁移器是最危险的形态**——它看起来能跑，所以不做。

若将来确实出现存量用户，正确做法是在**发布前**决定兼容策略；格式一旦发布即冻结，事后补迁移器属于二次迁移。

---

## 6. 代码改动清单

### 6.1 `assistant-crypto` / `assistant-vault` 层

> crate 布局说明：`assistant-security` 现只剩 `hibp` / `screen_lock` / `ssh_agent` / `unlock_dialog` / `clipboard_guard` / `reveal_guard`，其 `vault_crypto/mod.rs` 只有一行 `pub use assistant_crypto::vault_crypto::*;`。密码箱主体在 `assistant-vault`，密码学原语在 `assistant-crypto`。下表按现布局列出。

| 文件 | 改动 |
|---|---|
| `src-tauri/crates/assistant-crypto/src/vault_crypto/mod.rs` | 删除 `derive_master_key`（口令→根密钥）、`verify_master_password`、`derive_sqlcipher_key(MasterKey)`；新增 `derive_kek`、`derive_db_key(KEK, vault_id)`、`derive_entry_key(dek, entry_id)`、`derive_meta_key(dek)`。`assistant-security/src/vault_crypto/mod.rs` 只是转发，无需改 |
| `src-tauri/crates/assistant-vault/src/security_db/mod.rs` | 删除 `BOOTSTRAP_SALT`（:22-26，**无任何保留用途**，不留迁移例外）；`SQLCIPHER_PRAGMAS` 的 `kdf_iter` 改 `1`（:27-28，改前先用一条「旧 pragmas 建库 → `kdf_iter=1` 打开」的测试确认 raw key 下该 pragma 是否为 no-op）；`init`/`unlock` 改为接收 `dbKey`；`unlock` 删除第三次 Argon2id（`:154-180`）；新增 `UnsupportedVaultVersion` |
| `src-tauri/crates/assistant-vault/src/security_db/rekey.rs` | `change_master_password`（`:15`）改为 §3.2：不重加密条目，只重包 + `PRAGMA rekey`（当前 O(n) 重加密段在 `:73-207`，`PRAGMA rekey` 在 `:208`）；新增 `LocalDbKeyMismatch` 修复入口（旧口令重包 / 重建缓存，§3.5） |
| `src-tauri/crates/assistant-vault/src/security_db/biometric_ops.rs` | payload 由 64B（`BK‖MK`，长度校验 `:41-44`）改为 52B（`dek‖dbKey`）；新增 `fingerprint = SHA256(payload)` 存取与比对；`restore_keys_from_plaintext`（`:35-79`）改为返回 `(dek, dbKey)`，并补探针读（当前只做 `PRAGMA key` + `SQLCIPHER_PRAGMAS`，**无验证读**就写入 `conn`） |
| `src-tauri/crates/assistant-vault/src/security_db/entries/*.rs`、`crypto_ops.rs`、`vault_file/mod.rs` | `derive_entry_key(mk, id)` 共 **17 处 / 7 个文件**（`crypto_ops.rs` 6、`rekey.rs` 3、`write.rs` 2、`import.rs` 2、`vault_file/mod.rs` 2、`snapshot.rs` 1、`deleted.rs` 1），机械替换为 `dek` |
| `src-tauri/crates/assistant-vault/src/security_db/schema.rs`、`meta.rs` | `vault_meta` 表（当前只有 `key/value`，`schema.rs:4-7`）新增 `anchor_meta_hash`、`anchor_dek_version`、`anchor_revision`；删除 `master_key_salt` / `hash_salt` / `master_key_hash` 三个键及其读写函数（`meta.rs:51-70`） |
| `src-tauri/crates/assistant-vault/src/recovery_key/mod.rs` | **重写**：删除 `generate_recovery_key`、`decrypt_recovery_keys`、`unlock_with_recovery_key`；新增 `encode_paper(dek)` / `decode_paper(&str) -> Result<Dek, VaultError>`（含 Crockford + CRC32C + 归一化）、`RecoverySession` |
| `src-tauri/crates/assistant-vault/src/lib.rs` | 导出 `RecoverySession`；移除 `unlock_with_recovery_key` |

### 6.2 `vault_file` 与同步层（同步未启用）

| 文件 | 改动 |
|---|---|
| `src-tauri/crates/assistant-vault/src/vault_file/types.rs` | 新增 `VaultMetaFileV2`（`vaultId` / `dekVersion` / `slots` / `manifest` / `recoveryBackup` / `integrity`）；删除 `VaultCryptoMeta`（`masterKeySalt` / `hashSalt` / `argon2Params`）、`EmergencyDataFile`，不留兼容反序列化 |
| `src-tauri/crates/assistant-vault/src/vault_file/mod.rs` | `read_vault_meta`（`:260-263`）改为严格解析：`version=1` → `UnsupportedVaultVersion`（不迁移，§5），其余非法 → `Unknown`；删除 `read_emergency_data`（`:312`）、`write_emergency_data`（`:320-346`）、`clear_emergency_data`（`:347-366`）、`rewrite_vault_meta_with_new_key`（`:367`）与 `.unwrap_or(VAULT_META_VERSION)`（`:330`）；新增 `write_vault_meta_v2`（原子写）与 manifest 计算；`verify_vault_meta_integrity`（`:265`）改挂 `metaKey` |
| `src-tauri/crates/assistant-vault/src/sync_engine/pull_push.rs` | `remote_uses_other_master_key`（`:203`）改为比较 `vaultId` |
| `src-tauri/crates/assistant-vault/src/sync_engine/export.rs` | 删除 `to_crypto_meta`（`:88`）与 `VaultCryptoInfo` 的密钥字段（`masterKeySalt` / `hashSalt` / `argon2Params`）；`vaultId` 是否进入导出由 §10 D11 决定 |
| `src-tauri/crates/assistant-vault/src/sync_engine/emergency.rs` | 删除 `read_emergency_data`（`:24`）；`disaster_recovery_export`（`:36`）改为接受 `RecoverySession`，不再从恢复密钥字符串重建 `BK‖MK` |
| `src-tauri/crates/assistant-vault/src/sync_engine/mod.rs`、`preview.rs` | pull 时新增 manifest 校验（`VaultManifestMismatch`） |

**跨设备导出格式本期不设计**：同步未启用，`.vault-meta` v2 只需在单机 git 仓库内自洽。`VaultCryptoInfo` 一旦删掉密钥字段，导出里就不再有任何可重建 slot 的材料——这是有意的（§10 D11），不要顺手补一个「导出 salt」字段。

### 6.3 命令层与前端

| 文件 | 改动 |
|---|---|
| `src-tauri/src/cmd/vault/recovery.rs` | 现有 4 个命令：`vault_generate_recovery_key`（`:10`）、`vault_has_recovery_key`（`:42`）、`vault_verify_vault_meta_integrity`（`:50`）、`vault_disaster_recovery_export`（`:146`）。改为 `vault_export_recovery_key`（展示当前 `dek`，40 字符 + 分组）、`vault_recover_from_paper`（返回 `RecoverySession` 的导出结果）、`vault_rotate_dek`；`vault_generate_recovery_key` / `vault_has_recovery_key` / `vault_disaster_recovery_export` 删除（「是否已备份纸面」由 `recoveryBackup` 标记回答） |
| `src-tauri/src/cmd/vault/entries.rs` | `vault_change_master_password`（`:231`）内的 `disenroll_biometric`（`:248`）、`clear_emergency_data`（`:259`）、`RecoveryKeyInvalidated` 审计（`:281`）全部删除；改为「用新 `KEK` 重包生物识别 payload（`dek‖dbKey`）」，仅重包失败时 disenroll；`export_all_entries(state, true, &[])`（`:269`）的 `force=true` 删除——改密不再重写任何条目文件，`RecoverySession` 的导出路径也不需要 `force` 绕过 |
| `src/vault/unlock/RecoveryKeyForm.tsx` | **新建**（当前目录只有 `BiometricButton.tsx` / `DisasterRecovery.tsx` / `PasswordForm.tsx`）：纸面输入，实时归一化、分组显示、CRC 错误提示到组位 |
| `src/vault/unlock/DisasterRecovery.tsx` | 改为「纸面恢复」流程：输入 40 字符 → 只读预览 → 导出条目 |
| `src/vault/settings/RecoveryTab.tsx` | 「生成恢复密钥」改为「备份恢复密钥」（展示当前 `dek`）；新增「轮换数据密钥」独立入口 + 强警告；移除「恢复密钥已失效」的旧文案 |
| `src/vault/core/vault.service.ts` | 移除 `generateRecoveryKey`（`:262`）、`hasRecoveryKey`（`:265`）、`disasterRecoveryExport`（`:270`）；新增 `exportRecoveryKey` / `recoverFromPaper` / `rotateDek` |

### 6.4 必须同时删除的死代码

- `MasterKey` 作为「口令派生物」的所有用法（只保留其作为 `dek` 载体的角色）。
- `hash_salt` 与第三次 Argon2id 派生。
- `EmergencyData` / `emergency` 字段的全部读写路径（跨 `assistant-vault`、命令层、前端三层）。
- `unlock_with_recovery_key` 的 `bool` 返回与 `set_conn_and_key` 调用。
- v1 `.vault-meta` 兼容读取：`crypto` / `emergency` 反序列化、`masterKeySalt`、`hashSalt`、`argon2Params` 解析。
- v1 `integrity` HMAC 及其规范化编码（不保留兼容校验用途，理由见 §5）。
- 恢复码路径复制的 `PRAGMA` 字面量（`recovery_key/mod.rs:191`），统一走 `SQLCIPHER_PRAGMAS`。

本设计**不实现**迁移器，因此不应存在 staging 目录、`journal.json`、`.vault-legacy/` 归档、`LegacyVaultDetected` 这类代码——若在实现时被「顺手加上」，视为偏离设计。

---

## 7. 安全与可观测性

### 7.1 威胁矩阵

| 攻击者持有 | 能做什么 | v1 | v1.5 |
|---|---|---|---|
| 仓库 | 离线爆破口令 | 能（`BOOTSTRAP_SALT` 可预计算） | 能，但每库独立 salt + 参数可调 |
| 仓库 + 口令 | 解密全部条目 | 能 | 能 |
| 仓库 + 口令 + 本机磁盘 | 开库、读审计 | 能 | 能（`dbKey = HKDF(KEK)`） |
| **纸面** | 解密全部条目 | 能（`unlock_with_recovery_key` 还能开库，仅测试引用） | 能（**不能开库**，且无此函数） |
| 纸面 + 仓库 | 解全部条目 | 能 | 能 |
| 本机磁盘 + 口令 | 开库 | 能 | 能 |
| OS keystore | 生物识别解锁（等价于持有完整凭据：`dek` + `dbKey`） | 能 | 能 |
| 仓库历史（旧 meta + 旧口令） | 解当前条目 | 能 | **仍能**（C-1，只能靠 `rotate_dek` 吊销） |

按 §0.6 分档，这张表必须读成三句，而不是一句：

1. **T1 上是一进一出，不是净收益**。得到的是「改密不再静默作废纸面」（v1 的 `clear_emergency_data` 会让用户拿纸时才发现纸已废）；让出的是「改密吊销旧口令」（C-1：旧 meta + 旧口令仍能解出当前 `dek`）。两件事都落在用户唯一在意的数据上，方向相反、量级相当。
2. **T3 上是净收益，但分量有限**。纸面不再能开库、不再是一份可被复制的完整第二凭据。然而纸面已能解开全部条目（§0.6 推论 2），所以这条只挡住「用纸在本机把应用正常登录使用」，不构成数据泄露防线。
3. **T2 上是净收益**。改密 O(n)→O(1)、不再产生 n 文件 commit、随身零文件。代价是每台设备改密后各需一次 `rekey` 或重建（§3.5）。

**结论（据此修正旧表述）**：v1.5 不可替代的价值落在 **T2**——改密更便宜、更不容易让用户丢数据（纸面不会因改密而静默失效）。T1 上是等价交换，T3 上只是附带。**不要把它宣传成「更安全」**：在「旧口令 + 仓库历史」这个威胁上，它确实比 v1 更弱。

本表**只列持有型威胁**，不展开端点攻破（keystore 读取、签名获取、内核 / 管理员权限、物理接触），理由见 §0.5-7。生物识别在本设计中是**口令的等价替代凭据**，不是第二因素；开启后强度 = min(主密码, OS 登录)。

### 7.2 风险与缓解

| 编号 | 风险 | 缓解 |
|---|---|---|
| R1 | 纸面泄露 = 数据泄露 | 生成时强警告；建议离线保管；提供「轮换数据密钥」入口 |
| R2 | 纸面持有者可离线验证口令猜测（已知 `dek` 时每次猜测仍需 1 次 Argon2id + 1 次 GCM，与常规离线爆破**同价**，不加速） | 无实际增量：其已持有全部数据，猜出口令不带来任何新能力；不为它牺牲可抄写性 |
| R3 | `PRAGMA rekey` 中断损坏本机库 | 库是可重建缓存；审计双写 JSONL；`rekey` 前自动备份 |
| R4 | 改密不吊销旧口令 | UI 显式告知；提供 `rotate_dek` |
| R5 | 多设备改密后本机库失配 | §3.5 两条修复路径（旧口令重包 / 重建缓存） |
| R6 | 生物识别静默过期 | `fingerprint` 比对 → `BiometricReset` |
| R7 | 全熵密钥上的 `kdf_iter=256000` 纯浪费 | 改 `1` |
| R8 | **`rotate_dek` 不吊销生物识别**：wrapping key 由固定公开挑战的签名派生，是常量；`rotate_dek` 只换 payload 里的 `dek`，旧签名仍能解出新 payload。用户执行「轮换数据密钥」后会合理地认为旧凭据全部失效，实际没有 | `rotate_dek` 成功后**删除 Hello 凭据并强制重注册**（`delete_key` 已调用 `KeyCredentialManager::DeleteAsync`，路径现成）；若用户拒绝重注册，则视为「已停用生物识别」并显式告知 |
| R9 | **`set_key_unprotected` 默认回落为 `set_key`**（`biometric/mod.rs:24-26`）：非 Windows 路径把 wrapping key 与密文存入同一 key store；若两者 ACL 相同，AES-GCM 只是「任何人拿到都能解」的壳，等价明文 | 逐平台确认：macOS 必须设 `kSecAccessControlUserPresence`；Linux 必须确认 secret service 绑定会话锁；不满足的平台**直接禁用生物识别**，而不是静默降级 |
| R10 | 生物识别路径**成功解锁有审计、失败 / 取消无审计**（成功写 `VaultUnlock` + `"biometric"`，`biometric_ops.rs:79-84`；失败与 `UserCanceled` 无事件，口令路径的 `VaultUnlockFailed` 也不覆盖它） | 见 §7.3：成功 / 失败 / 签名变化全部写 JSONL |
| R11 | **纸面部分泄露 = 全文泄露**：CRC32C 让「已持有绝大部分 `dek`」的攻击者能以 1 次 CRC 验证候选，31³ ≈ 3 万次即可补全 3 个模糊字符（§4.4） | 不为此去掉 CRC（纠错是核心体验，且无 CRC 时同样可验证，见 §4.4）；改为在文案上要求「整张纸要么完整保密、要么视为已泄露」，禁止拍照局部留存 |

### 7.3 可观测性

- **审计双写**：`security_audit_log` 表 + 本机 JSONL（`.audit/*.jsonl`，不加密、不含密钥）。理由：恢复路径无法读库，若审计只在库里，则「用纸面导出全部条目」这件事不可审计。JSONL 让下次口令解锁时可以补录一条 `RecoveryExportUsed`。
- **恢复密钥使用可审计**：`RecoverySession::export_all` 结束时写一条 `RecoveryExportUsed { entryCount, at }` 到 JSONL。
- **改密/轮换必须留痕**：`MasterPasswordChange`、`DekRotated { from, to }`、`LocalDbRebuilt`、`AnchorRollbackRejected`。
- **生物识别路径必须留痕**：`BiometricUnlocked`、`BiometricFailed { reason }`（含 `UserCanceled`）、`BiometricSignatureChanged`、`BiometricEnrolled` / `BiometricRevoked`。理由：这条路径绕过了口令与 slot，且失败时当前完全不留痕（R10）。
- **审计位置**：`.audit/*.jsonl` 与 `security.db` 同级（本机数据目录），**不得落在 vault 目录内**（否则进 git，形成新的历史泄露面）；按大小 / 日期轮转，避免无限增长。

---

## 8. 实施阶段

| 阶段 | 内容 | 可交付性 |
|---|---|---|
| **P0-1** | 验证 raw key 下 `kdf_iter` 语义 → 改 `1`（`security_db/mod.rs:27-28`）；`recovery_key/mod.rs:191` 的 PRAGMA 字面量改为走 `SQLCIPHER_PRAGMAS` | 独立可合，不动密钥结构、不动 schema |
| **P0-2** | **一次原子切换**：引入 `dek` + `.vault-meta` v2（单槽）+ 严格解析（`version=1` → `UnsupportedVaultVersion`）；`init`/`unlock`/`change_password` 走新路径（改密 O(1)）；删 `BOOTSTRAP_SALT`、`hash_salt`、第三次 Argon2id；`derive_entry_key` 17 处换 `dek`；**同批删除 v1 恢复通道**（`generate_recovery_key` / `decrypt_recovery_keys` / `unlock_with_recovery_key`、命令层 `vault_generate_recovery_key` / `vault_has_recovery_key` / `vault_disaster_recovery_export`、前端对应入口） | 新库可用；旧库明确拒绝（§5）。**中间态不可发布**：P0-2 到 P0-3 之间没有恢复通道 |
| **P0-3** | 纸面编解码（Crockford + CRC32C + 归一化）+ `RecoverySession` + 命令层 `vault_export_recovery_key` / `vault_recover_from_paper` + 前端纸面入口 | 恢复通道重新可用，且权限已裁剪 |
| **P1-1** | `manifest` + `anchor`（含 `anchor_*` 写入 `vault_meta`） | 补上删除/回滚检测 |
| **P1-2** | 生物识别 payload 改 `dek‖dbKey`（52B）+ `fingerprint` + 自动重包 + **解锁验证读（探针，I12）** | 修掉静默过期与「假解锁」 |
| **P1-3** | `rotate_dek` + 全量重加密 + `dekVersion` + **删除 Hello 凭据强制重注册**（R8） | 真吊销能力 |

依赖关系：P0-2 依赖 P0-1；P0-3 依赖 P0-2；P1-1 依赖 P0-2；P1-2 依赖 P1-1；P1-3 依赖 P1-1（manifest 需全量重算）与 P1-2（重注册复用生物识别 payload 路径）。

**P0-1 只碰 `kdf_iter` 与 PRAGMA 去重，不碰 `BOOTSTRAP_SALT`**：删 `BOOTSTRAP_SALT` 会让 `generate_recovery_key` / `decrypt_recovery_keys` / `unlock_with_recovery_key` 全部编译失败，必须与 v2 切换同批，否则 P0-1 无法独立合入。

**没有「迁移」阶段，也没有「删除兼容分支」的收尾阶段**：v1 兼容代码在 P0-2 中一次性删完，不保留过渡期。

---

## 9. 测试计划

### 9.1 单元测试

| 编号 | 用例 | 断言 |
|---|---|---|
| T1 | `encode_paper` → `decode_paper` 往返（1000 次随机 `dek`） | 结果一致；长度恒为 40 |
| T2 | 归一化：小写、含 `-`/空格、全角、`O`/`0`、`I`/`1`/`l`/`L`、`U`/`V` 混用 | 全部解出同一 `dek` |
| T3 | 单字符替换（遍历 40 个位置 × 31 个其他字符） | 全部返回 `InvalidRecoveryKeyChecksum`（CRC-32 保证检出全部单字符错误） |
| T4 | 截断 / 超长 / 非法字符 | `InvalidRecoveryKeyFormat`，不 panic |
| T5 | CRC 正确的伪造 `dek`（攻击者构造） | 能通过 CRC，但解不开任何条目 → 证明 CRC 不是密钥预言机 |
| T5b | 纠错定位：随机挖掉 1~3 个字符后跑定位逻辑 | 唯一还原原 `dek`；且**不**触达任何口令相关代码路径 |
| T6 | AAD：把 `slotId`/`dekVersion`/`payloadKind` 任一改掉后解包 | `InvalidPassword`（GCM 失败） |
| T7 | `dbKey` 派生：同 `KEK` + 不同 `vaultId` | 输出不同 |
| T8 | `entryKey` 派生：同 `dek` + 不同 `entryId` | 输出不同 |

### 9.2 集成测试

| 编号 | 场景 | 断言 |
|---|---|---|
| I1 | 初始化 → 写入 100 条目 → 解锁 → 读出 | 全量一致；`dekVersion=1`；`recoveryBackup` 为空 |
| I2 | 改密（旧→新） | 条目文件 sha256 **未变**；`.vault-meta` 只变 slot 与 integrity；旧口令 `InvalidPassword`；纸面仍可恢复 |
| I3 | 改密后用旧 `security.db` 副本解锁 | `LocalDbKeyMismatch`；两条修复路径都能恢复可用状态 |
| I4 | `rotate_dek` | 条目全部重加密；`dekVersion+1`；旧纸面 → `InvalidRecoveryKey`；`dbKey` 未变（`security.db` 未被 rekey） |
| I5 | 用纸面恢复 | 返回 `RecoverySession`；**无法**获得 `dbKey` / 连接 / 审计（编译期即可验证） |
| I6 | 篡改 `manifest.count` | `VaultMetaIntegrityFailed` |
| I7 | 删除 `entries/e-05.enc` | pull 时 `VaultManifestMismatch` |
| I8 | 整块重放旧 `.vault-meta`（`dek` 未变） | 本机 `VaultRollbackDetected`；**同时确认**：不经本应用、直接取旧 meta + 旧口令仍能解出 `dek` → 记录为 C-1 的已知行为 |
| I9 | 改密后生物识别重包失败 | 下次生物识别解锁 → `BiometricReset`，不返回错误密钥 |
| I10 | 打开 `version=1` 的旧库 | `UnsupportedVaultVersion`；**不修改任何文件**（`.vault-meta` / `security.db` / `entries/` 的内容与 mtime 不变） |
| I11 | 重置仓库后重新初始化 | 新库 `version=2`、`vaultId` 重新生成；无旧文件残留；`recoveryBackup` 为空 |
| I12 | **生物识别解锁必须验证读**：`restore_keys_from_plaintext` 用 payload 的 `dbKey` 打开库后，**必须**执行探针查询（`SELECT value FROM vault_meta WHERE key='schema_version'`，与恢复码路径 `recovery_key/mod.rs:194-198` 一致） | 探针失败 → `BiometricReset`，**不得**进入已解锁状态。用例：用 A 设备的 `security.db` + B 设备的 keystore payload，断言返回 `BiometricReset` 而非「解锁成功、首次读条目才报错」 |
| I13 | `rotate_dek` 后生物识别 | 旧 Hello 凭据已被删除；旧 payload 无法解出新 `dek`；用户被要求重新注册 |

### 9.3 手工验收

1. 抄写 40 字符到纸面，再手输回来（3 人 × 3 次），统计抄错率与 CRC 定位准确率。
2. 打印页在 A4 与手机截图两种介质下可读性。
3. 「改密」与「轮换数据密钥」两个入口的文案是否让人混淆（重点：用户是否会以为改密能吊销旧纸面）。
4. 用一份开发机上的真实 v1 库验证：先备份 → 打开应用应报 `UnsupportedVaultVersion` 且文件未被修改 → 经「重置仓库」确认后能正常初始化（旧库不会被自动删除）。

---

## 10. 待决策项

| 编号 | 议题 | 现状 | 备选 |
|---|---|---|---|
| **D1** | 恢复密钥是否允许开库 | **否**（A3） | 允许 → 回到 v1 的病，不建议 |
| **D2** | `dbKey` 来源 | `HKDF(KEK)`，零文件 | 本机随机 + 本地包裹文件（改密免 `rekey`，但多一个文件，且需历史槽） |
| **D3** | `dek` 长度 | 20B / 160 bit | 32B / 256 bit（纸面 60 字符，抄写成本上升） |
| **D4** | 改密是否吊销 | **否** | 是 → 等价于每次改密都 `rotate_dek`（O(n)），不建议 |
| **D5** | 恢复密钥是否用助记词 | **否** | BIP39（需中文词表，见 §4.3） |
| **D6** | 是否提供「多凭据 slot」 | 暂不，结构已兼容 | 需要时加 YubiKey slot |
| **D7** | `rotate_dek` 是否也轮换 `dbKey` | **否**（两者无关） | 是 → 无收益，多一次 `rekey` |
| **D8** | 旧库是否迁移 | **否**（已决：无存量用户，不实现迁移器；v1 库直接 `UnsupportedVaultVersion` + 提示重置，§5） | 迁移 → 需 staging / journal / 回滚 / 崩溃注入，当前无收益 |
| **D9** | 审计主存储 | `security.db` + JSONL 双写 | 仅库 → 恢复路径不可审计 |
| **D10** | `vaultId` 是否随 `rotate_dek` 变 | **否**（仓库身份不变） | 变 → 会让同步判据失效 |
| **D11** | `.vault-meta` v2 的跨设备导出契约 | **本期不设计**（同步未启用；`VaultCryptoInfo` 删掉密钥字段后，导出不含任何可重建 slot 的材料，§6.2） | 启用同步时再设计：需决定 `vaultId` / `slots` 是否进导出、pull 时如何校验 manifest；届时 `remote_uses_other_master_key` 的 `vaultId` 判据才有实际作用 |

---

## 11. 行业对照

| 维度 | KeePassXC | Bitwarden | LUKS | **本设计 v1.5** |
|---|---|---|---|---|
| 数据密钥 | 无（口令派生） | 随机 `akey` | 随机 VK | 随机 `dek`（160 bit） |
| 凭据→数据密钥 | KDF transform | KDF(口令) 包 `akey` | 每 slot 独立 KDF | Argon2id → KEK → GCM 包 `dek` |
| 改密是否吊销 | 是（重加密） | 否（重包） | 否（重写 slot） | **否**（重包 + rekey 本机库） |
| 真吊销 | 换密钥 | 换 `akey` | `reencrypt` | `rotate_dek` |
| 恢复通道 | 无 | Emergency Access（账户接管） | keyfile slot（完整凭据） | **纸面 = `dek`，只能导出条目** |
| 恢复通道能否开库 | — | 能 | 能 | **不能**（`dbKey` 由 KEK 派生，不在纸面） |
| 随身额外文件 | — | — | keyfile（可选） | **零** |
| 仓库是否含口令 salt | 是 | 是（服务端） | 是（header） | 是（无法避免，见 §0.5） |

**本设计的两个原创取舍**（参考项目中均无对应实现）：

1. **恢复通道的权限半径由密钥结构裁剪，而非由 API 约定裁剪**：纸面里根本没有 `dbKey`，`RecoverySession` 也没有 `conn()`。
2. **随身零文件**：既不要 keyfile（LUKS 式），也不要恢复包（v1 式），也不要 `local-meta.json`（早期 v2 式）。

---

## 附录 A：术语

| 术语 | 定义 |
|---|---|
| `dek` | data encryption key，条目数据密钥，20 字节随机，跨设备共享，**纸面即此物** |
| `KEK` | key encryption key，`Argon2id(口令, salt)`，只用于包裹 `dek` 与派生 `dbKey` |
| `dbKey` | 本机 SQLCipher 页密钥，`HKDF(KEK)`，**不落任何文件** |
| `entryKey` | 单条目密钥，`HKDF(dek, entryId)` |
| `metaKey` | `.vault-meta` 完整性 HMAC 密钥，`HKDF(dek, "meta-integrity")` |
| slot | `.vault-meta` 中一条「用某凭据包裹 `dek`」的记录；当前只有 password slot |
| `dekVersion` | `dek` 代数，每次 `rotate_dek` +1，写入 AAD 与 manifest |
| `vaultId` | 仓库身份 UUID，首次初始化生成，永不变 |
| anchor | 本机记录 { metaHash, dekVersion, revision }，用于拒绝回滚 |
| 纸面 | 恢复密钥的物理/数字载体，内容就是 `dek` 的 40 字符编码 |

## 附录 B：被否决的两个方案

### B.1 双文件 keyslot（早期 v2）

**形态**：`.vault-meta`（同步槽：password / recovery 各包 `dek`）+ `local-meta.json`（本地槽：password / biometric 各包 `dbKey`），槽数组 + 2 代历史槽 + `payloadFingerprint`。

**否决理由**：

1. `local-meta.json` 是一个纯为「`dbKey` 需要被包裹」而存在的文件。而 `dbKey` 完全可以由 `KEK` 派生 —— 派生即得，无物可包，文件自然消失。
2. recovery slot 与 password slot 都包同一把 `dek`，本质是把「一把钥匙」存两遍。既然纸面可以直接承载 `dek`（40 字符够用），就不需要仓库里那份封装。
3. 槽合并规则、2 代历史槽、双槽一致性检查，全部是「为维持双文件而付出的复杂度」，与收益不成比例。
4. 多设备改密的修复流程需要人工介入历史槽；单槽方案只需输入一次旧口令（§3.5）。

**代价（诚实记录）**：双槽方案在「改密不 rekey 本机库」上更优；单槽方案把 `dbKey` 绑到 `KEK`，因此改密必须 `PRAGMA rekey`，**且每台设备各一次**（§3.5：N 台设备 = N 次修复）。权衡结果：本机库是可重建缓存，`rekey` 代价（单文件、有备份）小于「多一个文件 + 一套合并规则」——但这个判断只在**单机/双机**场景成立；设备数继续增长时，乘法项会反超，届时应重新评估 D2。

### B.2 密钥域外置（keyring out of repo）

**形态**：把 `.vault-meta` 从 git 移出，放进 OS keychain / 本机目录，仓库只留 `entries/*.enc` 与 manifest。

**为什么它是唯一能解决 §0.5 第 1 条（仓库即爆破靶）的方案**：仓库里没有 salt、没有包裹密文，就没有离线口令攻击面。

**否决理由（当前阶段）**：

1. **新手引导被破坏**：新设备 clone 仓库后无法解锁，必须走「配对」流程（扫码 / 口令 + 带外传递 `vault-meta`）。这是产品级成本，不是实现级成本。
2. 与「git 是唯一真相源」的既有心智冲突：用户会认为「我 clone 了就是备份了」，但实际没有。
3. 需要额外的一次性传递通道（二维码 / 短链），本质是把「恢复包」重新引入，只是换了个名字。

**结论**：这是**升级路径**，不是当前方案。若将来威胁模型上移（例如仓库可能被公开），应把它作为下一版的正式目标（附录 C）。

## 附录 C：未解决的根问题与升级路径

| 编号 | 根问题 | v1.5 的处理 | 真正解法 |
|---|---|---|---|
| G1 | 仓库含口令 salt + 包裹密文 ⇒ 离线爆破靶 | 每库独立 salt、Argon2id 参数可调、`kdf_iter=1` 去掉无用开销 | 密钥域外置（B.2） |
| G2 | 改密不吊销旧口令（C-1） | 显式产品语义 + `rotate_dek` 入口 | 密钥域外置后，`.vault-meta` 不再进历史，重包即可吊销 |
| G3 | 纸面泄露 = 数据泄露 | 强警告 + `rotate_dek` | 纸面 + 第二因素（但那就是「随身文件」，与 A2 冲突） |
| G4 | ~~恢复通道无法恢复审计 / 设置~~ | **不成立，已撤销**：审计、生物识别注册、设置、`revision` 均属 T3（§0.6），恢复通道精准覆盖 T1 的条目，这是刻意的范围界定而非缺口 | — |
| G5 | 本机库是缓存，重建后 anchor 基线重置 | 记录并提示 | anchor 外置到不可回滚位置 |

**升级触发条件**（写给未来的自己）：

- 若产品需要「公开仓库 / 仓库可能被第三方持有」→ 触发 G1，走 B.2。
- 若用户反馈「改密后旧纸面还有效，无法接受」→ 说明 D4 需要重新评估（代价是改密变 O(n)）。
- 若用户反馈「40 字符抄写太痛苦」→ 说明 D3 的 160 bit 选择需重新评估（但不要增加字符数，应改传递通道）。
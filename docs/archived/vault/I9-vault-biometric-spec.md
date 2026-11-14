# I9 密码箱 — 生物识别凭据规格（原 key-hierarchy-spec §5 迁出）

> 状态：规格（2026-11-11 分派）
> 来源：[I9-vault-key-hierarchy-spec.md](./I9-vault-key-hierarchy-spec.md) §5、§9.1 部分、§10 部分、§11.4 部分、附录 C-10–C-15、附录 D-11/D-13–D-18。
> 上游：[I9-vault-key-hierarchy-design.md](./I9-vault-key-hierarchy-design.md)
> 同层：[I9-vault-runtime-session-spec.md](./I9-vault-runtime-session-spec.md)（会话与锁）、[I9-vault-capability-gating-spec.md](./I9-vault-capability-gating-spec.md)（平台能力矩阵）
> 本规格只负责**生物识别凭据的 payload、keystore、注册/解锁/重包/吊销**。密钥层次本身见 key-hierarchy-spec。

---

## 0. 范围

涉及文件：

| 文件 | 改动性质 |
|---|---|
| `src-tauri/crates/assistant-vault/src/security_db/biometric_ops.rs` | 主改：payload、指纹、探针、审计、吊销 |
| `src-tauri/crates/assistant-vault/src/biometric/mod.rs` | trait 增加 ACL 判定方法；`delete_key` 错误语义 |
| `src-tauri/crates/assistant-vault/src/biometric/{windows,macos,linux}.rs` | ACL 判定；`delete_key` 区分 not-found / 真实失败 |
| `src-tauri/src/cmd/vault/biometric.rs` | 签名不变，跟随类型改名 |
| `src-tauri/src/cmd/vault/entries.rs` | 改密路径不再触碰生物识别（删除 v1 的 `disenroll` / 恢复材料清理调用） |
| `src/vault/unlock/BiometricButton.tsx`、`src/vault/core/slices/vaultBiometricSlice.ts`、`src/vault/settings/ChangePasswordTab.tsx` | 前端入口与重注册引导 |

不覆盖：密钥派生（key-hierarchy-spec §3）、会话锁定（runtime-session-spec）、平台能力总表（capability-gating-spec §4）。

---

## 1. 现状盘点

| 现有符号 | 位置 | v1.5 处理 |
|---|---|---|
| `KEY_ID_MASTER` / `KEY_ID_AES_KEY` / `KEY_ID_CIPHERTEXT` / `KEY_ID_SIG_HASH` | `biometric_ops.rs:14-17` | **改名**（§2.1）；**不保留旧名、不做 legacy 兼容** |
| `derive_wrapping_key_from_signature` | `:22-28` | **保留原样**，参数冻结（§3） |
| `compute_signature_hash` | `:30-35` | **保留原样**，哈希对象冻结（§3） |
| `restore_keys_from_plaintext` | `:37-88` | **重写**（§5） |
| `enroll_biometric_with_hint` | `:90-141` | **改内部**，签名不变（§7） |
| `unlock_biometric` | `:143-163` | **保留**（平台分派不变） |
| `disenroll_biometric` | `:166-180` | **改**：只删新 id（§11 B） |
| `is_biometric_enrolled` | `:183-197` | **改**：只查新 id（漏改 = 永远显示「未开启」） |
| `check_biometric_consistency` | `:199-221` | **改**：查新 id + 清理半注册残留（§5 约束 4） |
| `enroll_windows` / `unlock_windows` | `:226-290` | **改内部**：payload 与 key id；`vault-sig-hash` 校验、解密失败映射按 §6 |
| `enroll_separated` / `unlock_separated` | `:292-343` | **改内部**：payload 与 key id；条目缺失与解密失败映射按 §6 |
| `VaultCrypto::derive_sqlcipher_key(&bootstrap_key)` | `:55` | **删除**（§5 约束 3） |

---

## 2. key id 与 payload 布局

### 2.1 key id

payload 长度：**52B** = `dek(20) ‖ dbKey(32)`。v1 为 64B `BK‖MK`（校验在 `:42-45`）。

新增常量（`biometric_ops.rs` 顶部）：

```rust
const BIOMETRIC_PAYLOAD_LEN: usize = 52;
const BIOMETRIC_DEK_LEN: usize = 20;
```

key id **分平台**定义：

| 平台 | key id | 存什么 | 保护方式 |
|---|---|---|---|
| 共用 | `vault-payload` | 包裹后的 52B payload 密文 | OS keystore |
| 共用 | `vault-fingerprint` | `hex(SHA-256(payload))` **明文** | OS keystore |
| Windows | `vault-sig-hash` | `hex(SHA-256(signature))` | OS keystore |
| 非 Windows | `vault-wrapping-key` | `hex(随机 32B AES key)` | 平台 ACL（§10） |

旧 id（**仅作事实记录，代码中不出现**）：

```
vault-master-key / vault-master-aes-key / vault-master-ciphertext / vault-master-sig-hash
```

**改名是必须的，不是洁癖**：v1 的 `vault-master-key` 里存 64B `BK‖MK`。沿用旧名会让升级后首次生物识别解锁读到 64B 旧 payload，被判为长度错误；重新注册写新名、读旧名，形成**永久死循环**。改名后旧条目成为孤儿。

**不写 legacy 清理代码**。会留下旧条目的只有开发机；处理方式是手动删除同名系统凭据（Windows「设置 → 账户 → 登录选项」、macOS/Linux 钥匙串）。此说明只写在本规格，不进生产代码。

例外见 §11：`vault_reset_repository` 与 `rotate_dek` 产生的孤儿是新流程自产的，必须清理。

---

## 3. 签名与派生参数（冻结）

Windows 路径的挑战值、HKDF 参数、`vault-sig-hash` 的哈希对象都是**常量**。v1.5 **全部沿用 v1 取值，一个字节都不改**：

| 项 | 值 | 来源 |
|---|---|---|
| 挑战（challenge） | `SHA-256("assistant-vault-biometric-v1")` → 32B | `biometric/windows.rs` `biometric_challenge()`（`:101-105`） |
| 签名算法 | `KeyCredentialManager` 平台默认（TPM 或软件密钥对），不可配置 | Windows API |
| 签名调用 | `KeyCredential::RequestSignAsync(challenge)`，**每次调用都触发 Hello 验证** | `windows.rs` `request_signature`（`:227` 起） |
| HKDF 算法 | HKDF-SHA256 | `security_db/biometric_ops.rs` `derive_wrapping_key_from_signature`（`:22-28`） |
| HKDF salt | `b"assistant-vault-v1"` | 同上 |
| HKDF info | `b"aes-256-gcm-key"` | 同上 |
| HKDF 输出 | 32B，直接作 AES-256-GCM 密钥 | 同上 |
| `vault-sig-hash` | `lowercase_hex(SHA-256(signature))`，`signature` 为 `RequestSignAsync` 返回的**原始签名字节** | `compute_signature_hash`（`:30-35`） |

约束：

1. **挑战是公开且固定的常量，因此签名是确定性的**。`vault-sig-hash` 能比对成功本身就证明签名可复现；若签名每次不同，旧凭据根本无法解开新 payload，§9 的 R8 也不成立。
2. **`vault-sig-hash` 的哈希对象是原始签名字节**，不是 base64 文本、不是 HKDF 输出。任一端写错，症状是「TPM credential has changed」误报或「解密失败」，与真正的凭据变更无法区分。
3. **HKDF salt 保持 `assistant-vault-v1` 不变（已裁决）**。它是生物识别密钥派生域标签，不是 `.vault-meta` 格式版本（后者为 v2），两者不可混淆。v1.5 已用新 key id + 新 payload，无迁移需求，改名无收益、只增加「注册用 A、解锁用 B」的错配风险。
4. **非 Windows 路径没有签名，本节参数不适用**。其 wrapping key 是注册时 `rand` 生成的 32B 随机值，以 hex 存 `vault-wrapping-key`；payload 以 AES-256-GCM 加密后存 `vault-payload`。因此非 Windows **没有**「凭据已变更」信号，只能靠 ACL 门控（§10）。
5. **挑战字符串不进入 payload 加密的 AAD**。它只决定签名值；payload 完整性由 AES-256-GCM 认证标签承担。
6. **签名与 wrapping key 必须 zeroize**（`enroll_windows` / `unlock_windows` 现有代码已做，重写时不得丢失）。

---

## 4. fingerprint

```rust
fn compute_fingerprint(payload: &[u8]) -> String;   // 小写 hex(SHA-256(payload))
```

- 与 payload **一起**存 keystore（key id `vault-fingerprint`）。
- **明文 hex，不加密**——只用于「是否过期」的比对，SHA-256 单向，不含密钥信息。
- **必须对明文 52B payload 计算，不能对密文计算**：非 Windows 路径每次注册都会生成新的随机 wrapping key，密文随之变化；若指纹基于密文，则「payload 内容未变」也会被误判为过期，导致误报 `BiometricReset`。

---

## 5. `restore_keys_from_plaintext` 重写

```rust
fn restore_keys_from_plaintext(
    state: &SecurityDbState,
    payload: &VaultPlaintext,
    key_store: &dyn BiometricKeyStore,
) -> Result<(), VaultError>;
```

> 前置：本函数只处理**已解密的明文 payload**。签名校验与 payload 解密由平台入口完成（§6），失败在那里就地返回 `BiometricReset`，不会进入本流程。

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
7. 写入 session（dek / db_key / conn）
8. 审计 BiometricUnlocked（conn + JSONL）
```

约束：

1. **第 5 步探针读必须存在**。v1 只做 `PRAGMA key` + `SQLCIPHER_PRAGMAS` 就写入 `conn`（`:60-63`），产生「假解锁」（设计 I12）。探针失败必须返回 `BiometricReset`，**不得**进入已解锁状态。
2. **指纹检查（第 3 步）必须早于开库（第 4 步）**。若 payload 已过期（`dbKey` 是旧的），用旧 `dbKey` 开库必然让第 5 步探针失败，用户会看到「数据库损坏」而非「请重新注册」。
3. **禁止二次派生 SQLCipher 密钥**。v1 在 `:55` 调 `VaultCrypto::derive_sqlcipher_key(&bootstrap_key)`；v1.5 的 `dbKey` **本身就是页密钥**，直接 `PRAGMA key`。若保留该调用，口令路径用 `dbKey` 开库、生物识别路径用 `HKDF(dbKey)` 开库，**两条路径永远互相打不开**，且失败点在探针读 → 被误判成 `BiometricReset`，用户反复重新注册也无解。必须删除，并删除 `derive_sqlcipher_key` 的所有生产调用点（`security_db/mod.rs:77`、`mod.rs:130`、`biometric_ops.rs:55`、`recovery_key/mod.rs:173`）。
4. **`check_biometric_consistency` 的归属**。v1 在 `:76-88` 于开库前调用。v1.5 保留，但**位置提前到第 3 步之前**，语义改为：

   | 平台 | 判定 | 处理 |
   |---|---|---|
   | Windows | `has_key("vault-payload")` 为 false | 若 `has_key("vault-fingerprint")` 为 true，删之（清半注册残留）→ `BiometricReset` |
   | 非 Windows | `vault-wrapping-key` 或 `vault-payload` 任一缺失 | 删除三者（含 fingerprint）→ `BiometricReset` |

   它承担「半注册残留清理」职责，不能因为新增指纹检查就删掉——指纹条目存在但 payload 缺失时，第 3 步会走到「读不到 payload」而无法区分原因。

5. **任何失败都不得写入 session**。第 3–6 步的 `conn` 只能是局部变量，成功后才 `mem::swap` 进 `state`。
6. **第 3/5 步的失败审计只能写 JSONL**。这两处 `conn` 不可用（未打开或探针已失败），必须走 `audit::log_dual(None, Some(writer), ..)`。JSONL 写入失败只 `warn`，**不阻断**生物识别解锁（避免审计目录不可写造成永久锁死）。
7. **指纹不是防回滚机制**。攻击者若能同时替换 payload 与 fingerprint，指纹依然匹配。防回滚由第 6 步 anchor 承担。文档与代码注释都必须写明这一点。

---

## 6. 平台解锁入口

### 6.1 Windows（`unlock_windows`）

```
1. blob = get_key("vault-payload")
   └─ 条目缺失 ──▶ BiometricReset
2. signature = request_signature("vault-payload")        // 此处弹 Hello
   └─ BiometricUnavailable ──▶ 原样返回，不审计 BiometricFailed
3. sig_hash = compute_signature_hash(signature)          // §3
4. stored = get_key("vault-sig-hash")
   ├─ 条目缺失           ──▶ 审计 BiometricFailed{reason:"sig_hash_missing"}（JSONL）→ BiometricReset
   └─ 值与 sig_hash 不符 ──▶ 审计 BiometricFailed{reason:"sig_changed"}（JSONL）→ BiometricReset
5. wrapping_key = HKDF-SHA256(signature, HKDF_SALT, HKDF_INFO)   // §3
6. plaintext = decrypt_field(blob, wrapping_key)
   └─ 失败 ──▶ 审计 BiometricFailed{reason:"decrypt_failed"}（JSONL）→ BiometricReset
7. zeroize(signature) / zeroize(wrapping_key)
8. restore_keys_from_plaintext(state, &plaintext, key_store)   // §5
```

### 6.2 非 Windows（`unlock_separated`）

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
6. restore_keys_from_plaintext(state, &plaintext, key_store)   // §5
```

约束：

1. **Windows 的 `vault-sig-hash` 校验必须早于解密**。凭据被重建后，payload 密文未变、指纹仍匹配，但新签名派生的 wrapping key 解不开旧密文。先解密只会给出「解密失败」；先比对 sig-hash 才能给出「凭据已变更，请重新注册」。
2. **解密失败必须映射为 `BiometricReset`，不得保留 `VaultError::Crypto`**。v1 在 `:279-280`、`:335-337` 用 `map_err(VaultError::Crypto)`，前端无法据此给出重新注册引导。
3. **`request_signature` 返回 `BiometricUnavailable` 时不得审计 `BiometricFailed`**。这是用户主动取消或凭据不存在，不是异常。
4. **这两个函数在失败时不得写入 session**（与 §5 约束 5 同一要求）。
5. **`BiometricReset` 的语义是「payload 已不可用，需重新注册」**，上层**不得**把它当作「解锁失败」自动重试——重试不会成功，只会重复弹 Hello。UI 必须直接切换到「重新注册」引导。

---

## 7. 注册

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

理由：该回调在 `src-tauri/src/cmd/vault/biometric.rs:73-80` 用于广播 `BIOMETRIC_ENROLL_STEP = "credential-created"`，让 UI 在「系统凭据已创建、等待指纹验证」时切换文案。Windows 上首次注册需用户抬手验证，无此事件会卡在无反馈的等待态。

行为：

```
1. 平台 ACL 判定（§10）──▶ 不满足则 BiometricUnavailable（不写任何条目）
2. dek = session.dek，db_key = session.db_key      // 任一缺失 → Locked
3. payload = dek ‖ db_key（52B）
4. fingerprint = compute_fingerprint(payload)
5. key_store.set_key("vault-payload", 包裹后的 payload)
   └─ Windows: 内部先 ensure_credential，成功后调 on_credential_ensured()；
              加密密钥 = HKDF-SHA256(sig, HKDF_SALT, HKDF_INFO)
   └─ 非 Windows: 生成随机 32B wrapping key → set_key("vault-wrapping-key") → 加密 payload
6. key_store.set_key("vault-fingerprint", fingerprint.as_bytes())
   └─ Windows 额外 set_key("vault-sig-hash", compute_signature_hash(sig))
7. 审计 BiometricEnrolled（conn + JSONL）
```

约束：

1. **先写 payload 再写 fingerprint**。反序会在中途失败时留下「指纹在、payload 无」的残留，被 §5 约束 4 判为 `BiometricReset`——虽能自愈，但会多一次无谓的注册失败提示。
2. 注册是**幂等**的：重复注册直接覆盖，不要求先反注册。
3. 非 Windows 路径每次注册都生成**新的**随机 wrapping key，`vault-wrapping-key` 必须与 `vault-payload` 同时更新。
4. **不做旧 key id 清理**（§2.1）。注册路径只碰 4 个新 id。
5. **注册与解锁必须使用同一套挑战与 HKDF 参数**（§3）。

---

## 8. 自动重包（已废弃）

`dbKey = HKDF(dek, vaultId)`（key-hierarchy-spec §3.2 / 附录 C-16）之后：

| 场景 | 密钥变化 | 处理 |
|---|---|---|
| 改密 | `dek` / `dbKey` **均不变** | payload 仍有效，**无需重包** |
| `rotate_dek` | `dek` / `dbKey` 均变 | 走 §9 的「删除凭据」，由用户重新注册 |

因此不再需要 `repackage_biometric_payload`，也不再有「重包失败 → 标记待重注册」这条路径。**实现时不要保留该函数**；若未来出现「密钥变化但必须保留凭据」的场景，再按当时的安全语义重新设计。

`fingerprint`（§3）仍保留：它用于识别「payload 与当前会话密钥不匹配」（例如手工替换了 keystore 文件、或跨设备拷贝了 payload），此时返回 `BiometricReset` 而不是用错误密钥开库。

---

## 9. R8：`rotate_dek` 强制重注册

```
1. key_store.delete_key("vault-payload")
   ├─ 成功 ──▶ 继续
   └─ 失败 ──▶ 审计 BiometricFailed{reason:"delete_failed"}（JSONL）
              ──▶ 返回错误，rotate_dek 不得宣告成功
2. 依次删除 vault-fingerprint / vault-sig-hash / vault-wrapping-key（失败只 warn）
3. 审计 BiometricRevoked
4. UI 提示「轮换数据密钥后需重新注册生物识别」
5. 用户拒绝重注册 ──▶ 视为「已停用生物识别」，显式告知
```

理由：wrapping key 由固定公开挑战的签名派生，是常量；`rotate_dek` 只换 payload 里的 `dek`，旧签名仍能解出新 payload。**不删凭据 = 旧凭据仍然有效**。

约束：

1. **第 1 步失败必须中止**。这是全流程唯一「删除失败等于安全前提落空」的位置。
2. **`rotate_dek` 的执行顺序**：删除凭据必须是 `lifecycle::rotate_dek` 的**第 1 步**（任何破坏性写入之前）。若放在末尾，条目已按 `dek_new` 重加密，此时删除失败而中止，会留下「新密文 + 旧 meta」的不一致仓库。
3. 第 2 步的 3 个条目失败只 `warn`：`vault-payload` 已删时，剩余条目无法构成可用解锁路径。
4. **不得**把失败降级为「标记待重注册」——§8 的自动重包机制已废弃；payload 过期只有「删除凭据 + 重新注册」一条路径（§9）。

---

## 10. R9：`set_key_unprotected` 与 ACL

`biometric/mod.rs:24-26` 的非 Windows 路径把 wrapping key 与密文存入同一 key store。要求逐平台确认 ACL：

| 平台 | 要求 | 不满足时 |
|---|---|---|
| macOS | `kSecAccessControlUserPresence` | **禁用生物识别** |
| Linux | secret service 绑定会话锁 | **禁用生物识别** |
| Windows | Hello（TPM） | 已满足 |

**禁止静默降级**：`set_key_unprotected` 回落路径必须在编译期或运行时显式判定；判定不通过时 `enroll_biometric_with_hint` 返回 `BiometricUnavailable`，而不是「存了但任何人都能解」。

---

## 11. 重置与轮换的 keystore 清理

### 11.1 `vault_reset_repository` 必须清空 keystore

若不清，重置后会出现：

```
旧 vault-payload 仍在
  └─ is_biometric_enrolled() 返回 true
     └─ UI 显示「已开启生物识别」（而新库从未注册过）
        └─ 用户点生物识别解锁
           └─ 指纹比对：旧 payload 与旧 fingerprint 自身一致 → 通过
              └─ 用旧 dbKey 开新库 → 探针读失败
                 └─ BiometricReset → 「请重新注册」
```

要求：

```rust
fn clear_all_biometric_entries(key_store: &dyn BiometricKeyStore) -> Result<(), VaultError>;
```

- 删除全部 4 个新 id：`vault-payload` / `vault-fingerprint` / `vault-sig-hash` / `vault-wrapping-key`。
- 调用时机：`vault_reset_repository` 删除文件**之前**（先清 keystore 再删文件）。
- 清理失败**必须中止重置并报错**。
- 审计 `BiometricRevoked`（`conn = None`，走 JSONL）。

### 11.2 `delete_key` 必须能区分「不存在」与「删除失败」

| 平台 | 现状 | 必须改为 |
|---|---|---|
| macOS（`macos.rs:135-145`） | 已正确：not-found → `Ok(())`，其余 → `Err` | 不变 |
| Windows（`windows.rs:150-160`） | **全部错误被丢弃**，恒 `Ok(())` | not-found → `Ok(())`；其余 → `Err(BiometricFailed)` |
| Linux（`linux.rs:225-248`） | `let _ = clear_sync`，恒 `Ok(())` | `error.is_null()` 或 GError 为 not-found → `Ok(())`；其余 → `Err` |

理由：§9 与 §11.1 都要求「删除失败即中止」；若 `delete_key` 恒返回 `Ok`，该约束在任何平台都无法生效，T-B1/T-B3 退化为永远通过的假测试。

### 11.3 `disenroll_biometric` 只删新 id

删除 4 个新 id，**不含**旧 id。旧条目属开发机环境残留，不在产品逻辑内。

### 11.4 分工

| 场景 | 删除范围 | 失败处理 |
|---|---|---|
| `rotate_dek`（§9） | `vault-payload` 优先，其余 3 个尽力 | `vault-payload` 失败 → **中止**；其余失败 → warn |
| `vault_reset_repository`（§11.1） | 全部 4 个 | 任一失败 → **中止重置** |
| `disenroll_biometric`（§11.3） | 全部 4 个 | 失败 → 返回错误（用户显式操作，不能假装成功） |

---

## 12. 命令层与前端

### 12.1 命令

`src-tauri/src/cmd/vault/biometric.rs` **签名不变**，只跟随类型改名：

| 命令 | 说明 |
|---|---|
| `vault_is_biometric_available` | 平台能力探测 |
| `vault_is_biometric_enrolled` | 只查新 id |
| `vault_unlock_biometric` | 平台分派；`BiometricCancelled` 原样返回 |
| `vault_enroll_biometric` | 保留 `credential-created` 事件 |
| `vault_disenroll_biometric` | 只删新 id |

### 12.2 前端

| 组件/切片 | 改动 |
|---|---|
| `src/vault/unlock/BiometricButton.tsx` | 「重新注册」引导；`BiometricReset` 不自动重试 |
| `src/vault/core/slices/vaultBiometricSlice.ts` | 注册/解锁状态；`reset` 语义 |
| `src/vault/settings/ChangePasswordTab.tsx` | 删除「同时更新生物识别」提示（改密不改变 payload） |
| `src/i18n/locales/*/vault.json` | `vault.biometric.reset_required` / `reenroll_required` |

---

## 13. 错误码

| 错误码 | 触发 | retryable |
|---|---|---|
| `BiometricUnavailable` | 平台不可用 / ACL 不满足 | 否 |
| `BiometricCancelled` | 用户取消 | 否（不自动重试） |
| `BiometricFailed` | 注册/签名/解密异常 | 否 |
| `BiometricReset` | payload 过期/缺失/指纹不符 | 否（必须重新注册） |

---

## 14. 测试

| 编号 | 用例 | 断言 |
|---|---|---|
| T-B1 | `rotate_dek` 时注入 `delete_key("vault-payload")` 失败 | `rotate_dek` 返回错误，`dekVersion` 未递增，`dek` 未落盘 |
| T-B2 | `vault_reset_repository` 后 | `is_biometric_enrolled()` 为 false；4 个 key id 全部不存在 |
| T-B3 | 注入 `clear_all_biometric_entries` 失败 | 重置中止；`.vault-meta` / `security.db` / `entries/` 均未删除 |
| T-B4 | Windows 下改 `vault-sig-hash` | 解锁返回 `BiometricReset`；审计 `sig_changed`；`restore_keys_from_plaintext` 未被调用 |
| T-B5 | 篡改 `vault-payload` 使 GCM 失败 | 解锁返回 `BiometricReset`，不是 `VaultError::Crypto` |
| I12 | 用 A 设备 `security.db` + B 设备 keystore payload | `BiometricReset`，不得进入已解锁状态 |
| I13 | `rotate_dek` 后 | 旧 Hello 凭据已删除；旧 payload 无法解出新 `dek`；要求重新注册 |
| T-B6 | keystore payload 与当前 `dek` 不匹配（跨设备拷贝 / 手工替换） | `BiometricReset`，不得用错误密钥开库 |

T-B1/T-B2/T-B3 通过 mock `BiometricKeyStore` 注入失败；前提是 `delete_key` 具备可失败语义（§11.2）。

测试落点：

- 新增 `src-tauri/crates/assistant-vault/tests/biometric.rs`（mock key store；不依赖真实 Hello/TouchID）；
- `I12`（跨设备 payload）可复用 `tests/security_db.rs` 的临时库夹具；
- 前端「`BiometricReset` 后强制重新注册」用例放入 `src/vault/core/__tests__/vaultBiometricSlice.test.ts`（新增）。

---

## 15. 禁止事项

1. **禁止**为 v1 生物识别 key id 写任何读写/清理代码。
2. **禁止**在 `rotate_dek` 中把 `vault-payload` 删除失败降级为 warn。
3. **禁止**在 `dek` 落盘之后才删除生物识别凭据。
4. **禁止** `delete_key` 吞掉非 not-found 错误后返回 `Ok`。
5. **禁止**改动生物识别挑战值、HKDF salt / info、`vault-sig-hash` 的哈希对象。
6. **禁止**在 `unlock_windows` 中先解密再校验 `vault-sig-hash`。
7. **禁止**把生物识别 payload 解密失败映射为 `VaultError::Crypto`。
8. **禁止**在平台能力不满足时静默降级并声称已开启。
9. **禁止**在 `vault_reset_repository` 中跳过 `clear_all_biometric_entries`。

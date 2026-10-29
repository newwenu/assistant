# 后端 Crate 架构依赖分析

> 审查日期：2026-10-27
> 审查范围：`src-tauri/crates/` 下 9 个子 crate + 主 bin 之间的内部依赖关系、职责分布、下放可行性
> 审查方法：逐 crate Cargo.toml + 全量源码引用检索

---

## 1. 现状：依赖图与职责

### 1.1 内部依赖图（现状）

```
config (叶)    i18n (叶)    orb (叶)    tools (叶)    crash (叶)
providers (叶)  ← 已消除 security 依赖（凭证注入方案）
storage (叶)    ← 已消除 i18n 依赖（结构化错误 + 主 app 翻译桥接）
agent   → providers, storage  ← 已消除 i18n 依赖（结构化错误 + 主 app 翻译桥接）
security (叶)   ← 零内部依赖，但持有 6 类互不相关的重依赖（见 §6.1）
sync    → security
主 app  → 全部 10 个 crate
```

无循环依赖，DAG 结构清晰。共 3 条内部边。

**但这张图掩盖了真正的问题**：`security` 看似叶节点，实际同时持有 `rusqlite`(SQLCipher)、`reqwest`、`arboard`、`zxcvbn`、`ssh-key`/`rsa`、`keyring` 六类互不相关的重依赖，**任何消费者都必须全付**。`sync` 只用其中几个加密原语，却被拖入全部重依赖。这是 §6.1 要解决的核心，与"边数多少"无关。

### 1.2 各 crate 源码规模

| Crate | 行数 | 说明 |
|---|---|---|
| assistant-providers | ~8400 | 最大，含 3 个 Provider 适配 + 流处理 + 模型目录 |
| assistant-security | ~7000 | 6 个领域混合（见 §6.1） |
| assistant-sync | ~2700 | Vault 文件格式 + 同步编排 + Git 传输（职责混杂，见 §2.4） |
| assistant-storage | ~2200 | 会话/消息/Provider/附件 SQLite |
| assistant-agent | ~1400 | 聊天编排 + 请求构建 + 压缩 |
| assistant-tools | ~700 | OS 工具注册与执行 |
| assistant-config | ~500 | JSON 配置读写 |
| assistant-orb | ~400 | 悬浮球几何/位置计算 |
| assistant-i18n | ~100 | 国际化翻译查找 |
| assistant-crash | ~100 | 崩溃转储 |
| **主 app** | **~6600** | 含 protocol/dto ~1100 行 + cmd 逻辑 |

### 1.3 各 crate 职责与外部依赖量

| Crate | 职责 | 外部依赖数 | 评价 |
|---|---|---|---|
| assistant-config | JSON 配置读写 | 4 | 🟢 轻 |
| assistant-i18n | 国际化翻译查找 | 2 | 🟢 轻 |
| assistant-orb | 悬浮球几何/位置计算 | 2 | 🟢 轻（tauri 依赖见 §6.2，收益已降级） |
| assistant-tools | OS 工具注册与执行 | 6+平台 | 🟢 轻 |
| assistant-security | 加密、安全数据库、剪贴板守卫、生物识别、SSH、密码强度 | 27+平台 | 🔴 过重（6 类重依赖叠于一身，见 §6.1） |
| assistant-storage | 会话/消息/Provider/附件 SQLite 存储 | 10 | 🟡 中等 |
| assistant-sync | Vault 文件格式 + 同步编排 + Git 传输 | 8 | 🟡 中等（编排误置于此，见 §2.4） |
| assistant-providers | LLM Provider 流式调用（已无 API Key 加解密，见 §2.3） | 12 | 🟡 中等 |
| assistant-agent | 聊天编排、请求构建、压缩 | 8 | 🟡 中等 |

### 1.4 编译时间基准（dev profile, 20 并发, x86_64-pc-windows-msvc）

> 数据来源：`cargo build --timings`，2026-10-27，总耗时 171.5s，617 个编译单元

| 编译目标 | 耗时 | codegen | 说明 |
|---|---|---|---|
| assistant-app (lib) | 33.5s | 25.1s (75%) | 🔴 最慢，主 app 体积大 |
| assistant-storage | 11.2s | 9.1s (81%) | rusqlite + 大量泛型 mono |
| assistant-providers | 10.7s | 5.4s (50%) | 3 个 Provider 适配 + 流 |
| zxcvbn（security 传递） | 9.7s | 8.7s (90%) | 🔴 纯 codegen 爆炸，security 专有 |
| assistant-config | 5.1s | 4.4s (87%) | |
| arboard（security 传递） | 4.1s | 0.8s (19%) | security 专有 |
| assistant-i18n | 3.2s | 2.6s (82%) | |
| rusqlite | 3.2s | 1.6s (49%) | security + storage 共享 |
| ssh-key（security 传递） | 3.6s | 2.2s (62%) | security 专有 |
| assistant-security | 3.0s | 1.2s (39%) | crate 本体反而不慢 |
| assistant-sync | 1.5s | 0.4s (25%) | |
| assistant-agent | 1.3s | 0.7s (50%) | |
| assistant-orb | 1.3s | 0.7s (56%) | |
| assistant-tools | 0.7s | 0.3s (43%) | |

**security 传递依赖编译时间**：zxcvbn(9.7s) + arboard(4.1s) + ssh-key(3.6s) + rsa(1.8s) + argon2(1.0s) + aes-gcm(1.0s) + chacha20poly1305(0.4s) + keyring(0.3s) + rusqlite(3.2s) ≈ **25.1s**（**这是各单元编译耗时之和，不是关键路径墙钟时间**；rusqlite 与 storage 共享，属重复计入）。

**~~providers → security 传递影响~~**：providers 曾因 re-export security，其消费者被迫等待上述依赖编译完成。**已消除**（§2.3 凭证注入）。

**⚠️ 关于"减少 25s"的正确表述**（与 §2.4、§6.6 同一标准）：
- **`cargo build -p assistant-providers`**：真实受益——不再编译 security 的 25.1s 传递依赖。
- **完整 workspace 构建**：**零收益**——主 app 仍依赖 security，这些依赖照常编译。且 20 并发下，25.1s 是并行编译单元耗时之和，不等于墙钟时间缩短 25s。
- 故"编译闭包减少 25s"只能理解为**该 crate 单独构建时的依赖闭包缩减**，不能表述为整体构建提速。

---

## 2. 内部依赖边逐条审查

### 2.1 ~~storage → i18n~~ ✅ 已消除

**原用途**：conversations/ops.rs 和 agent_modes.rs 中用 `assistant_i18n::t()` 生成错误消息。

**原问题**：存储层不应决定用户看到的语言。i18n 是表现层关注点，不应出现在数据层。

**实施方案**：
- storage 定义 `StorageError` 枚举（thiserror），`#[error(...)]` 放 i18n key（如 `conversation.not_found`、`agent_mode.builtin_cannot_delete`）
- 加 `StorageError::other(msg)` 辅助构造方法
- 主 app `cmd/mod.rs` 新增 `translate_storage_error(e: StorageError) -> String`，匹配变体调用 `assistant_i18n::t()` 翻译
- 消费点（conversation.rs、agent_mode.rs、chat/insert.rs）加 `.map_err(translate_storage_error)`
- storage 内部 `Result<_, String>` 的函数通过 `From<String> for StorageError` 自动 `?` 转换

**收益**：storage 变为叶 crate，减少 1 条依赖边。

### 2.2 ~~agent → i18n~~ ✅ 已消除

**原用途**：multimodal.rs 中校验附件限制的错误消息，compaction.rs 中默认摘要提示词。

**原问题**：业务逻辑层不应硬绑翻译。

**实施方案**：
- agent 定义 `AgentError` 枚举（thiserror），变体含 i18n key + 参数化变体（`TooManyImages { max }`、`ImageTooLarge { max_mb }`）
- 加 `AgentError::other(msg)` 辅助构造方法
- `admit_image`/`admit_file` 返回类型从 `Result<_, String>` 改为 `Result<_, AgentError>`
- 主 app `cmd/mod.rs` 新增 `translate_agent_error(e: AgentError) -> String`，匹配变体调用 `assistant_i18n::t()` / `t_with_args()` 翻译
- 消费点（chat/mod.rs）加 `.map_err(translate_agent_error)`
- compaction.rs 的 `default_compact_summary_prompt()` 改为纯英文常量（给 LLM 的 prompt 无需 i18n）

**收益**：agent 减少 1 条依赖边。

### 2.3 ~~providers → security（1 处 re-export）~~ ✅ 已消除

**原用途**：providers/src/crypto.rs 整个文件仅 re-export security::crypto 的 5 个函数（encrypt/decrypt API key、get_machine_id）。

**原问题**：providers 为 5 个函数依赖 security 整个 crate（27 个外部依赖），传递拉入 rusqlite、argon2、zxcvbn、arboard 等无关依赖。

**实际采用的方案：凭证注入（优于原文档的方案 A/B）**

代码现状（已核实）：
- providers/src/crypto.rs 已删除，providers Cargo.toml 无 assistant-security，源码零 `assistant_security` 引用
- `ProviderConfig`（providers）**不含 api_key 字段**——providers 只负责 HTTP 流式请求
- agent 的 `build_chat_request(..., api_key: String)` 把 api_key 作为**入参**接收，只持有不解密
- 全部 5 处加解密调用集中在主 app：`cmd/provider.rs` ×3（掩码/保存/解密）、`cmd/compaction.rs` ×1、`cmd/chat/mod.rs` ×1
- providers 与 agent 各 **0 处**加密调用

```
主 app cmd/chat/mod.rs
  ├─ crypto::get_machine_id() + decrypt_api_key_b64(&provider_row.api_key_encrypted)
  └─ build_chat_request(..., api_key)   ← 明文参数注入
        └─ agent::ChatRequest { api_key: String }
              └─ providers 发 HTTP 请求（完全不知道加密存在）
```

**为什么优于原方案 A/B**：

| 维度 | 方案 A（拆 assistant-crypto） | 方案 B（providers 自实现 50 行） | 实际方案 C（凭证注入） |
|---|---|---|---|
| 新 crate | ➕1 | 0 | **0** |
| providers 是否叶 | ❌ 依赖 crypto crate | ✅ | **✅** |
| 代码重复 | 0 | ❌ 复制加密逻辑 | **0** |
| 加密逻辑归属 | crypto crate | ❌ 分散到 providers | **security 单一归属** |
| 职责合理性 | 尚可 | ❌ providers 承担密钥管理 | **✅ 加密归 security，网络归 providers** |

**方案 A/B 的错误前提**：两者都假设「providers 需要加解密能力」，因而为其寻找更轻的获取路径。但实际架构中 providers 只发 HTTP 请求、`ProviderConfig` 连 api_key 字段都没有，**根本不需要加密能力**。方案 B 尤其有害——会让 providers 承担密钥派生、machine-id 绑定、AES-GCM 参数选择等安全职责，造成安全逻辑分散（比依赖边更严重的问题）。

**⚠️ 注意区分**：方案 A 的错误是「**让 providers 依赖 crypto crate**」，而不是「crypto crate 本身不该存在」。§6.1 仍建议建立 `assistant-crypto`（服务 sync / vault / security），只是 providers 不在其消费者之列。

**唯一权衡**：明文 api_key 短暂存在于主 app 内存并作为参数传给 agent。但这不可避免（发 HTTP 时明文必然在内存），且实际方案更收敛——明文只在调用链参数中出现，不进入 config 层层传递，暴露面小于"config 携带密钥"。

**收益**：providers 变为叶 crate。`cargo build -p assistant-providers` 的依赖闭包缩减约 25s（§1.4）；**完整 workspace 构建零提速**（主 app 仍依赖 security）。架构上消除了"网络层依赖密钥管理层"的错误方向。已达成。

### 2.4 sync → security（耦合集中于 `sync_engine` + `vault_file` 两个模块，必须整体迁移）

**用途**：sync 在 4 个模块中引用 security，性质分四类——
- 加密原语：`MasterKey` / `CipherAlgorithm` / `VaultPlaintext` / `VaultCrypto`
- Vault 领域类型：`VaultCategory` / `VaultEntryImport` / `VaultEntrySnapshot` / `VaultEntryExport` / `CustomFieldInput`（归 vault）
- 共享错误枚举：`VaultError`（按 §6.1 判定归 `assistant-crypto`，拆分后 sync 直接依赖 crypto 即可）
- **数据库状态**：`SecurityDbState`（含 `recovery_key::EmergencyData`）

**实测分布（百行精度）**：

| 模块 | 行数 | 引用 security | 性质 |
|---|---|---|---|
| `vault_file/` | ~500 | 加密原语 + Vault DTO | 文件格式与加解密 |
| `sync_engine/` | ~1000 | 加密原语 + Vault DTO + **`SecurityDbState`** | 同步编排，**17 处 DB 调用** |
| `git_sync/` | ~900 | ❌ 零引用 | 纯 Git 传输 + SSH |
| `types/` | ~200 | ❌ 零引用 | 自有 DTO |
| 合计 | ~2700 | — | — |

**关键事实（上一版本严重低估）**：`SecurityDbState` 在 `sync_engine/mod.rs` 中作为函数参数出现 **23 处**，对 db 的实际调用 **17 处**：

| 方法 | 次数 | 读/写 |
|---|---|---|
| `get_master_key()` | 8 | 读 |
| `vault_crypto_info()` | 3 | 读 |
| `list_entries()` / `export_all_snapshots()` / `list_deleted_entry_ids()` / `get_entry()` | 4 | 读 |
| `upsert_imported_entries()` | 1 | **写** |
| `restore_from_history_tx()` | 1 | **写**（事务内回滚） |

**上一版方案不成立的原因**：「只把 `export_all_entries` 迁出、sync 只收入参数据」最多只能消掉 2/17（`get_master_key` + `list_entries` 各一）。剩余 15 处属于 `prepare → pull → merge → push → finalize` 的**多步交互编排**，每步中间都读写库；远端导入（`upsert_imported_entries`）与历史回滚（`restore_from_history_tx`）本质就是写库。**编排与数据库不可分**，无法靠"预取数据"消除。

**因此真实选项只有两个**：

| 方案 | 做法 | 代价 | 结果 |
|---|---|---|---|
| **A（推荐）** | `sync_engine/` + `vault_file/` 整体迁入 `assistant-vault` | ~1500 行迁移 + 23 个签名 + 主 app 4 个文件调用点改写 | ✅ 边真正消除，`assistant-sync` 变纯叶 |
| **C** | 接受 `sync → security` | 0 | 边保留，`-p assistant-sync` 仍须全付 security 重依赖 |

**⚠️ `vault_file` 必须与 `sync_engine` 同批迁移，否则成环**：
- `vault_file/mod.rs` 引用 `VaultEntryImport` / `VaultEntrySnapshot` / `VaultCategory` / `CustomFieldInput`
- 按 §6.1 这些 DTO 归 `assistant-vault`
- 若 `vault_file` 留在 sync、`sync_engine` 迁入 vault → `vault → sync`（用 vault_file）同时 `sync → vault`（用 DTO）→ **循环依赖**

结论：**`vault_file` 与 `sync_engine` 同去同留**。这也说明 `assistant-sync` 的真实定位是「Git 传输基础设施」，Vault 文件格式本属 vault 领域。

**迁移后 `assistant-sync` 的形态**：只剩 `git_sync/` + `types/`（~1100 行），**零内部 crate 依赖**，成为纯叶节点。主 app 侧仅剩 `set_ssh_persist_hosts` 一个调用点，无影响。

**收益（需区分构建场景）**：
- **单独编译 `-p assistant-sync`**：闭包从 security 全量重依赖降为 8 个轻依赖（serde/chrono/base64/sha2/hmac/uuid/log/thiserror），实测减少约 20s（zxcvbn 9.7s + arboard 4.1s + ssh-key 3.6s + rsa 1.8s + rusqlite 3.2s + reqwest）。
- **完整 workspace 构建**：**无提速**（主 app 仍依赖全部 crate）。
- 真正普适的收益是依赖归属清晰：`-p assistant-vault` 可独立编译（crypto + sync + SQLCipher + keyring，**无网络栈、无 OS-UI 栈**）。

**备选（不拆 crate）**：security 加 `vault-crypto` feature，sync 用 `default-features = false`。受 feature unification 限制，仅在单独编译 sync 时有效（见 §6.6 同类问题），**且无法解决 `SecurityDbState`**，故不推荐。

### 2.5 agent → providers（类型 + 流调用）

**用途**：agent 构建请求和消费流式响应，必须调用 providers 的 stream_chat 接口，同时引用 ChatDelta、ProviderError、ProviderConfig 等类型。

**评价**：本质依赖，agent 的核心职责就是"用 providers 处理 LLM 流"。不可消除。

### 2.6 agent → storage（类型 + 数据读取）

**用途**：agent 构建 ChatRequest 时需要从 storage 读取 ConversationDetail、ProviderRow、AgentModeRow、AttachmentStore。

**评价**：本质依赖，agent 的编排逻辑需要数据源。不可消除。

---

## 3. 主 app 职责分析：可下放至子 crate 的功能

主 app 当前 6001 行（40 个 .rs 文件），其中约 1/3 是可下放的领域逻辑。

### 3.1 🔴 强烈建议下放

| 功能 | 当前位置 | 行数 | 目标 | 条件 |
|---|---|---|---|---|
| DTO/协议类型定义 | protocol/dto/*.rs | 1069 | 新 crate assistant-protocol（**⚠️ 有条件**，见 §3.1.1） | 需先断开对 storage/providers 的类型引用（见 §4.1）；**且必须同时补齐 5 个直出类型的 DTO + 引入 TS 代码生成，否则抽 crate 无真实收益，不如留在主 app** |
| ~~崩溃转储~~ | ~~crash_dump.rs~~ | 87 | ~~新 crate assistant-crash~~ ✅ 已创建 | 完全独立，已落地（`crates/assistant-crash`） |
| 存储分析逻辑 | cmd/storage.rs | 159 | assistant-storage | 需剥离 tauri AppHandle 胶水，核心逻辑纯 DB + attachment_store |
| 会话工具函数 | cmd/conversation_utils.rs | 38 | assistant-storage | 纯 DB 查询，返回 storage 类型，主 app 做 DTO 转换 |

### 3.1.1 protocol/DTO 消费者核查（决定是否值得抽 crate）

**核查结论（已全量 grep）**：
- 子 crate 对 DTO 的引用 = **0 处**（`crates/` 下无任何 `Dto` 引用）
- 子 crate 内的内联 serde struct（`sync/vault_file/types.rs`、`security/types/mod.rs`、`storage/conversations/*`）是**各自领域类型**，非 DTO 镜像
- 唯一名字重叠：providers 的 `ProviderErrorPayload` → DTO `ProviderErrorPayloadDto`（`From` 包装）

**反方向的事实更重要**：5 个子 crate 类型**直接跨 IPC 到前端，完全绕过 DTO**：

| 命令 | 返回类型 | 有 DTO？ |
|---|---|---|
| `vault_sync` | `assistant_sync::SyncResult` | ❌ |
| `vault_get_sync_config` | `assistant_sync::SyncConfig` | ❌ |
| `vault_sync_readyness` | `assistant_sync::Readyness` | ❌ |
| vault recovery | `assistant_sync::vault_file::IntegrityResult` | ❌ |
| `vault_ssh_list` | `Vec<assistant_security::ssh_agent::SshKeyInfo>` | ❌ |

**真实架构状态**：
```
前端 TS 类型（手写镜像，chatTypes.ts / vault.types.ts）
   ↑ camelCase 契约
DTO（主 app 内部，rename_all=camelCase + deny_unknown_fields）──┐ 部分覆盖
子 crate 领域类型 ─────────────────────────────────────────────┘ 5 个类型直出前端
```

**因此 DTO 不是"共享契约层"，而是主 app 内部的序列化适配层，单消费者。** 它不可删除（承担 camelCase 塑形 + `deny_unknown_fields` 校验），但"协议定义中心"的说法不成立——只覆盖部分 wire 类型。

**对 §3.1 抽取 assistant-protocol 的影响**：
| 原声称收益 | 实际 |
|---|---|
| 主 app 精简 1069 行 | 仅行数搬家，总量不变 |
| DTO 可复用 | **0 复用**（无第二消费者） |
| 强制纯度约束 | ✅ 真实，但**只约束一部分**，契约仍分裂 |
| 编译收益 | 0（主 app 仍依赖全部 crate） |

**⚠️ 只按原方案抽取，会得到一个"半截契约 crate"**——禁止 DTO 依赖 storage/providers，但 sync/security 的 5 个 wire 类型仍在外直出，反而制造"契约已集中"的假象。

**建设性方案（抽 crate 的前提条件）**：
1. **补齐契约**：为 5 个直出类型建立 DTO（`SyncResultDto`/`SyncConfigDto`/`ReadinessDto`/`IntegrityResultDto`/`SshKeyInfoDto`），主 app 做 From 桥接 → wire 契约 100% 集中
2. **代码生成**：引入 `ts-rs` 或 `typeshare` 从 Rust 生成 TS 类型。当前前端是手写镜像，且 [protocolContract.test.ts](../../src/chat/core/__tests__/protocolContract.test.ts) 只校验 TS 自身字段、**不校验 Rust 侧**，手写漂移无任何保护
3. 只有 1+2 同时做，抽 crate 才有真实收益（编译器保证前后端契约一致）；**否则不抽**，保留在主 app 作为序列化适配层（P3）

### 3.2 🟡 可下放（需少量重构）

| 功能 | 当前位置 | 行数 | 目标 | 条件 |
|---|---|---|---|---|
| Chat 消息插入 | cmd/chat/insert.rs | 258 | assistant-agent | insert 是编排逻辑（协调会话查询、分支插入、provider 校验、active branch 更新），属于 agent 职责域；**前置条件**：① 5 处 `assistant_i18n::t()` 调用需改为结构化错误（与 §2.1/2.2 同模式）；② `extract_attachment_ids` 引用 `assistant_providers::types::parse_content`，agent 已依赖 providers，无需额外处理 |
| API Key 掩码 | cmd/provider.rs | — | ❌ **留在主 app**（原建议已过时） | providers/src/crypto.rs 已删除（§2.3 消除）。当前 `mask_api_key_encrypted` 在 cmd/provider.rs 内直接调用 `assistant_security::crypto`（decrypt_api_key_b64 + get_machine_id），主 app 已依赖 security，搬运无收益 |
| Vault 编排辅助 | cmd/vault/mod.rs | — | ❌ **不搬运**（原建议"→ assistant-security + 定义 VaultConfig trait"已撤回） | 核实 [mod.rs](../../src-tauri/src/cmd/vault/mod.rs#L1-L60)：`ssh_bridge_from_conf()` 读 `AppConf::load()`、`ssh_remove_all_on_lock(app: &AppHandle)` 用 `app.state::<SshAgentState>()`——是**配置读取 + tauri 状态 + 领域 API 调用的编排胶水**。搬入 security 需 security 依赖 config **和** tauri，并用 trait 解耦——这正是 §2.4 已否定的"用抽象固化放错位置的职责"。且 security 当前**零内部依赖、零 config 引用**，是完美叶节点，不应污染。命中 §4.3 ⑤，留在主 app |

### 3.3 🟢 必须留在主 app

| 功能 | 原因 |
|---|---|
| setup.rs | 编排所有子系统初始化 |
| window.rs | 窗口创建/定位，依赖 tauri |
| tray.rs | 系统托盘，依赖 tauri |
| protocol/events.rs | emit_broadcast 依赖 AppHandle.emit() |
| cmd/config.rs 副作用 | 全部驱动 tauri 窗口/插件行为 |
| cmd/orb.rs 吸附动画 | 直接操作 tauri 窗口位置 |
| cmd/mascot_asset.rs | ❌ 原建议"→ 新 crate assistant-mascot"**不成立**：两个入口均为 `#[command]` 且直接持有 `AppHandle`（取 `app_data_dir()`），独立后新 crate 仍须依赖 tauri（重依赖），并把 tauri 引入一个本应轻量的叶 crate；且无第二消费者、无重依赖、仅 138 行。命中 §4.3 检查表 ⑤，留在主 app |

---

## 4. 下放依赖链影响分析

### 4.1 🔴 会加重依赖链的下放（必须避免）

**resolve_context_window_for_conversation → assistant-storage**

该函数同时引用 assistant_storage 和 assistant_providers。若移入 storage，storage 必须新增 providers 依赖，导致 storage → providers 链式拉入（**注：providers 已不依赖 security，见 §2.3，故不再有三段链**）。storage 是基础 crate，未来可能有更多消费者，不应依赖网络层。

处理方式：留在主 app，或改为参数注入——storage 只返回原始数据，主 app 组合 providers 的计算结果。

**assistant-protocol 如果依赖内部 crate**

当前 protocol/dto 已经依赖了 3 个内部 crate（与"仅依赖 serde"的预期不符）：
- chat.rs → `assistant_providers`（ProviderErrorCode、ProviderErrorPayload、RetryNotification、TokenUsage）、`assistant_storage`（ConversationRow、MessageRow、TokenUsageSummary）
- settings.rs → `assistant_storage`（AgentModeRow）
- branch.rs → `assistant_storage`（SiblingInfo）
- config.rs → `assistant_config`（AppConf、OrbConf、MascotConf — From impl 桥接）

其中 config.rs 的 `From<assistant_config::*>` 是**纯桥接**（DTO 字段与 config struct 1:1 映射），断开方式与 storage/providers 相同：protocol 内部重定义同字段 struct，From impl 留主 app。

若 protocol 直接依赖 storage + providers + config，则任何依赖 protocol 的 crate 都会传递拉入整条链。**必须先断开**：protocol 内部重定义 DTO 字段（不再 From impl 引用 Row 类型），仅依赖 serde + serde_json，From impl 留在主 app 做桥接。

**~~vault 编排 → assistant-security~~**（原设想：若 security 直接依赖 config，则 sync → security → config 新增传递，故用 trait 解耦 config）

**已否决**：security 当前**零内部依赖、零 config 引用**，是纯叶节点。搬入 vault 编排（`AppConf::load()` + `AppHandle` state）会让 security 同时依赖 config 与 tauri，再用 trait 抽象去"化解"这个自造依赖——与 §2.4 否定的 trait 注入是同一反模式。**正确做法：编排留在主 app**（读配置、注入数据、调领域 API），security 保持叶节点，无需 trait。

### 4.2 🟢 不会加重依赖链的下放

| 下放项 | 目标 | 新增依赖 | 传递影响 |
|---|---|---|---|
| assistant-protocol | 新 crate | 仅 serde + serde_json（需先断开 Row 类型引用） | 零 |
| assistant-crash | 新 crate | crash-handler + minidump-writer + dirs + chrono | 零（无消费者） |
| 存储分析 | assistant-storage | 无 | 零 |
| 会话工具 | assistant-storage | 无 | 零 |
| Chat 插入 | assistant-agent | 无（agent 已依赖 storage） | 零 |

> ~~API Key 掩码 → assistant-providers~~ 已从本表移除：providers/src/crypto.rs 已删除，掩码逻辑在主 app 直接调用 security，搬运无收益（见 §3.2）。
> Mascot 逻辑已从本表移除：其真实依赖不止 `config + dirs`，还包含 **tauri**（`#[command]` + `AppHandle`）。若独立成 crate 会把 tauri 拉入新叶节点，并非"零传递影响"。详见 §3.3 与 §4.3。

### 4.3 独立 crate 判断标准（决策检查表）

下放前先过一遍检查表，**命中任一条件**才值得独立 crate；全部不命中则留在主 app：

| # | 条件 | 判据 | 本项目示例 |
|---|---|---|---|
| ① | 多消费者 | 类型/函数被 ≥2 个 crate 引用 | config、storage |
| ② | 可脱离 tauri 独立测试 | 核心逻辑不依赖 `AppHandle`/窗口系统，且可测逻辑 >100 行 | crash（纯文件 IO，已有测试）；orb 已满足此条（tests/placement.rs、tests/hover.rs 已存在，见 §6.2），故不构成独立理由 |
| ③ | 编译隔离 | 拉入重依赖（单项编译 >3s），需 feature gate 让开发时跳过 | security（zxcvbn 9.7s）、crash |
| ④ | 独立领域 + 体量 | 自有类型体系与错误处理，且 >300 行 | sync（1483）、tools（697） |
| ⑤ | 以上均不命中 | 留在主 app：tauri 胶水 / 单消费者 / 轻依赖 / 体量小 | **mascot_asset（138）** |

**关键区分：「不能放进某个 crate」≠「必须新建 crate」。**

以 mascot 为例，原论证是"不应放入 config（config 职责是 JSON 读写）"，这一步成立；但由此推出"必须新建 assistant-mascot"是跳跃——漏掉了第三个选项"留在主 app"。判断时应先问：**该模块是否本质上是 tauri 框架胶水？** 若是，则主 app 就是它的正确归属（§7.2 定义主 app 只放 tauri 胶水），独立只会把 tauri 重依赖搬进新 crate。

**何时应该重新评估 mascot**：出现以下任一变化时再考虑独立——① agent 或其他 crate 需要读取 mascot 资产路径/元数据（命中 ①）；② 核心逻辑改为接收 `PathBuf` 而非 `AppHandle`，可纯 Rust 测试（命中 ②）；③ 引入 image / Rive 运行时等重依赖（命中 ③）；④ 扩展到动画状态机/资源打包/热更新，超过 300 行（命中 ④）。

---

## 5. workspace.dependencies 策略

### 5.1 现状问题

根包 [workspace.dependencies] 有约 58 个外部依赖（已逐个核实消费者），其中：
- **32 个仅 1 个子 crate 使用**（含 20 个 security 独占：aes-gcm、chacha20poly1305、argon2、hkdf、zeroize、subtle、data-encoding、machine-uid、hex、rand、arboard、sha1、zxcvbn、keyring、windows-future、core-foundation、security-framework、libsecret、ssh-key、rsa）
- **9 个仅主 app 使用**（6 个 tauri-plugin + rustc_version_runtime + mouse_position + csv）
- windows crate 开了 14 个 feature（已核实 Cargo.toml:94-112），但 assistant-tools 只需 `Win32_Foundation` + `Win32_Storage_FileSystem` 两个（见 §5.3）

> 注：文档原估"约 20 仅 1 个 / 约 11 仅主 app"偏低，实际为 32 / 9。security 独占依赖是最大集中区，与 §6.1 拆分高度相关——拆分后这些依赖会**自然**落在新 crate 的 Cargo.toml（拆分副产品），但**不必为此单独做 workspace 精简**（见 §5.2 修正）。

### 5.2 workspace.dependencies 策略（⚠️ 收益有限，不建议激进精简）

| 类别 | 处理方式 |
|---|---|
| ≥2 个 crate 共享 | 保留在 workspace.dependencies |
| 仅 1 个子 crate 使用 | **可保留**（集中版本管理，未来加消费者无需搬迁） |
| 仅主 app 使用 | 同上，保留亦可 |
| 仅 dev-deps 使用 | 保留 workspace（统一测试依赖版本） |

**⚠️ 修正上一轮建议**：原"把 32 个独占依赖移到各 crate、直接写版本号"**收益有限且可能反噬**：
1. `workspace.dependencies` 的核心价值是**版本单点管理**，与"被几个 crate 使用"无关。为单一消费者保留在 workspace 是 Rust 社区常见做法。
2. 移出后版本号分散在多个 Cargo.toml，升级需逐处修改；一旦出现第二个消费者又要搬回，产生 churn。
3. 唯一收益是根 Cargo.toml 更短，纯可读性。

**唯一可做的精简**：§5.3 的 windows crate（14 feature 写死在 workspace → 改为 workspace 只留版本、成员按需声明）。**但收益同样需限定**：仅 `cargo build -p assistant-tools` 受益，完整 workspace 构建因 feature unification 取并集而零提速（详见 §5.3）。故它是**可维护性整理（P3）**，不是编译优化。其余"32 个独占依赖归位"应视为**可选整理**，不列为 P2。

**与 §6.1 的关系**：若执行 §6.1 拆分，新 crate 的 Cargo.toml 本就要写依赖，此时可顺势把相关依赖直接写在新 crate 内（而非 workspace），这是拆分的自然产物，不必单独规划搬运。

### 5.3 windows crate feature 问题（⚠️ 收益有限，机制需更正）

**现状（已核实 [Cargo.toml:94-112](../../src-tauri/Cargo.toml#L94-L112)）**：`[workspace.dependencies]` 中 `windows = { version = "0.62", features = [...] }` 声明了 **14 个 feature**。消费者仅 **2 个**：`assistant-tools`（`windows = { workspace = true }`）与 `assistant-security`（同上），主 app 不直接使用。

**⚠️ 更正原文档的机制描述**：「workspace 继承不支持 feature 叠加/覆盖」**前半句错误**。Cargo 实际规则是：
- **叠加支持**：成员可写 `windows = { workspace = true, features = ["Win32_Foundation"] }`，features 与 workspace 声明的**取并集**
- **削减不支持**：无法从成员侧移除 workspace 已声明的 feature，也无法把 `default-features = true` 改为 `false`
- 因此问题不是"不能叠加"，而是**workspace 把 14 个 feature 写死，tools 被迫接受 security 的全部需求**

**正确修法（优于原文档的"移出 workspace"）**：
- workspace 中只保留版本：`windows = "0.62"`（不写 features）
- 各成员按需声明：`windows = { workspace = true, features = [...] }`
- 这样**既保住版本单点管理**（§5.2 的核心价值），又实现 feature 按需，无需把 windows 移出 workspace

**各成员实际需要（已核实源码）**：
- `assistant-tools`：`Win32_Foundation` + `Win32_Storage_FileSystem`（[fs_link.rs](../../src-tauri/crates/assistant-tools/src/os/fs_link.rs) 的 HANDLE/CloseHandle/GetFinalPathNameByHandleW）
- `assistant-security`：`Win32_Foundation`、`System_Registry`、`Graphics_Gdi`、`UI_WindowsAndMessaging`、`UI_Controls`、`UI_HiDpi`、`System_WinRT`、`System_LibraryLoader` + `Security_Credentials`、`Security_Cryptography`、`Storage_Streams`（biometric 走 WinRT）、`UI_Input_KeyboardAndMouse`、`Graphics_Dwm`、`System_Power`、`System_RemoteDesktop`、`Win32_Security`、`Storage_FileSystem`（unlock_dialog/screen_lock/biometric 使用面广）

**⚠️ 收益限定（与 §2.4/§6.6 同一标准）**：
- **`cargo build -p assistant-tools`**：真实受益——不必为 tools 编译 security 才需要的 Gdi/Registry/WinRT 等模块
- **完整 workspace 构建**：**零收益**——feature unification 会取并集，security 需要几乎全部 14 个 feature，tools 仍然拿到同样的并集
- 故本项是**依赖声明精确性**的整理（可维护性），不是全量构建提速。**降级为 P3 可选**

---

## 6. 其他问题

### 6.1 assistant-security 职责过重（~7000 行 / 27 依赖 / 6 领域）

**现状**：一个 crate 同时持有 6 类互不相关的重依赖，消费者无法按需取用：

| 重依赖 | 承载模块 | 编译耗时 |
|---|---|---|
| `rusqlite`（SQLCipher） | `security_db/`（~2300 行） | 3.2s |
| `reqwest` | `vault_crypto/hibp.rs`（~100 行） | ~5s（含网络栈） |
| `arboard` | `clipboard_guard.rs`（~100 行） | 4.1s |
| `zxcvbn` | `password_strength/`（~150 行） | 9.7s |
| `ssh-key` + `rsa` | `ssh_agent/`（~500 行） | 5.4s |
| `keyring` | `biometric/`（~700 行） | 0.3s |

**模块体量（百行精度）**：

| 模块 | 行数 | 外部依赖特征 |
|---|---|---|
| `security_db/` | ~2300 | rusqlite(SQLCipher) |
| `unlock_dialog/` | ~1400 | 平台 UI（windows/macos/linux） |
| `biometric/` | ~700 | keyring + 平台 API |
| `vault_crypto/` | ~600 | 纯加密原语（`hibp.rs` 除外） |
| `ssh_agent/` | ~500 | ssh-key + rsa |
| `screen_lock/` | ~400 | 平台 API |
| `types/` | ~350 | 仅 serde |
| `recovery_key/` | ~200 | 依赖 security_db |
| `password_strength/` | ~150 | zxcvbn |
| `clipboard_guard.rs` | ~100 | arboard |
| `audit/` | ~100 | rusqlite |
| `reveal_guard.rs` | ~50 | 零依赖 |
| `crypto/` | ~100 | aes-gcm + machine-uid |

**拆分设计：2 个新 crate、1 个缩小、0 个取消**

```
assistant-crypto   (叶)   ~700 行
  vault_crypto/{mod,types,totp}    ~500   Argon2/HKDF/AES-GCM/ChaCha20/TOTP
  crypto/api_key                   ~100   API Key 加解密（machine-id 绑定）
  error.rs                         ~100   VaultError（见下方归属判定 3/4）
  deps: aes-gcm chacha20poly1305 argon2 hkdf sha2 hmac sha1
        base64 data-encoding zeroize subtle rand machine-uid hex thiserror

assistant-sync     (叶)  ~1100 行
  git_sync/                        ~900
  types/                           ~200
  deps: serde serde_json log thiserror chrono base64 sha2 hmac uuid
  ← vault_file + sync_engine 已迁出（§2.4），零内部 crate 依赖

assistant-vault    (中层) ~5100 行
  security_db/                     ~2300
  sync_engine/   ← 自 sync 迁入     ~1000
  biometric/                       ~700
  vault_file/    ← 自 sync 迁入     ~500
  types/（VaultEntry* DTO）          ~250
  recovery_key/                    ~200
  audit/                           ~100
  deps: crypto, sync, rusqlite(SQLCipher), keyring, uuid, chrono, serde, log

assistant-security (中层) ~2700 行
  unlock_dialog/                   ~1400
  ssh_agent/                        ~500
  screen_lock/                      ~400
  password_strength/                ~150
  clipboard_guard.rs                ~100
  hibp/  ← 自 vault_crypto 迁入       ~100
  reveal_guard.rs                    ~50
  deps: crypto, arboard, zxcvbn, ssh-key, rsa, reqwest
```

**目标依赖图（无环）**：

```
crypto (叶)
sync   (叶)
vault    → crypto, sync
security → crypto
主 app   → 全部
```

内部边从 1 条（`sync → security`）增至 3 条。**边数增加不是退步**：拆分的目的是**缩小每个 crate 的依赖闭包**，让轻消费者不再被迫拉入六类重依赖；主 app 闭包不变（本就依赖全部 crate）。

**四条关键归属判定（均已按代码核实）**：

1. **`hibp` 必须从 `vault_crypto/` 物理挪出到 security**。它是该目录下唯一使用 `reqwest` 的成员；若随 crypto 一起走，"纯加密原语叶 crate"立刻变成网络 crate，`-p assistant-crypto` 与 `-p assistant-sync` 的重依赖收益全部归零。职责上与 `password_strength` 同属"密码强度/泄露检测"。

2. **`biometric` 必须进 vault，不能留 security**。`security_db/biometric_ops.rs` 是其唯一调用方。crate 依赖是**传递闭包**（不按模块裁剪），若 biometric 留 security 而 vault 需要它，则形成 `vault → security`，数据层被动拉入 `reqwest` + `arboard` + `zxcvbn` + `ssh-key`，层次倒置。

3. **`VaultError` 归 `assistant-crypto`，不归 vault**（⚠️ 上一版本判断错误）。`VaultError` 不是 Vault 业务 DTO，而是 security crate 的**共享错误枚举**，被 19 个文件引用；其中 `unlock_dialog`（9 处）、`ssh_agent`（4 处）、`reveal_guard`（2 处）、`clipboard_guard`（1 处）都属于**拆分后仍留在 security 的模块**。若按上一版把 `types/` 整体归 vault，security 剩余模块将**反向依赖 vault**，直接破坏 `vault → security` 方向。`VaultError` 自身只依赖 `vault_crypto::types::CipherAlgorithm`，放入 crypto 无新增边。

4. **`types/` 必须拆两半**：

   | 内容 | 归属 | 依据 |
   |---|---|---|
   | `VaultError` | `assistant-crypto` | 共享错误枚举，跨模块使用，非业务 DTO |
   | `VaultEntryCreate/Update/Snapshot/Summary/Detail/Decrypted/Export`、`VaultCategory`、`CustomFieldInput`、`ImportOutcome`、`VaultCryptoInfo`、`RevealResult` | `assistant-vault` | 真正的 Vault 领域 DTO |

**为什么 security 不并入 vault**：拆分后 `assistant-security` 的 7 个模块只被 `cmd/vault/*` 调用（单消费者）。但若合并，vault 闭包将重新包含 `reqwest` + `arboard` + `zxcvbn` + `ssh-key` + `rsa`，`-p assistant-vault` 的独立编译价值丧失。**保留独立 crate 是为了把 OS/网络依赖隔离在 vault 之外**，不是为了多消费者。

**遗留债务（须记录，不应默认正确）**：`VaultError` 放入 crypto 后，名称与所在 crate 语义不符（crypto 不含 Vault 概念）。建议后续更名为 `SecurityError`：crate 内 25 处 + 主 app 3 文件 10 处，成本可控。更名前文档与代码均按 `VaultError` 处理。

**收益（须限定场景）**：
- `-p assistant-sync`：闭包从 security 全量重依赖 → 8 个轻依赖，约 -20s（详见 §2.4）
- `-p assistant-vault`：crypto + sync + SQLCipher + keyring，**无 reqwest/arboard/zxcvbn/ssh-key**，约 -20s
- `-p assistant-security`：crypto + reqwest + arboard + zxcvbn + ssh-key/rsa，**无 SQLCipher/keyring**
- **完整 workspace 构建：零提速**（主 app 仍依赖全部 crate；且各单元耗时之和 ≠ 墙钟缩短）

### 6.2 assistant-orb 不应依赖 tauri（⚠️ 收益已修正）

orb 是纯几何/位置计算，库 crate 依赖 GUI 框架不符合分层。**但"无法独立测试"是伪需求**（下文核实）。若要移除 tauri，在 orb 中定义纯 Rust 类型，由调用方做转换。

**精确耦合点（已核实）**：仅 [placement.rs:16](../../src-tauri/crates/assistant-orb/src/placement.rs#L16) 的 `get_available_screens(app: &AppHandle)` 一个函数使用 tauri。`geometry.rs`、`hover.rs` 完全是纯 Rust（无任何 tauri import）。

**最小改动方案**：将 `get_available_screens` 移出 orb（移到主 app 的 cmd/orb.rs），orb 只保留 `ScreenBounds` 输入参数 + 纯计算函数。orb Cargo.toml 删除 tauri 依赖即可。

**⚠️ 原"无法独立测试"的理由不成立**：orb 已有 [tests/placement.rs](../../src-tauri/crates/assistant-orb/tests/placement.rs) 与 [tests/hover.rs](../../src-tauri/crates/assistant-orb/tests/hover.rs)，测试对象是纯计算函数（`ensure_orb_on_screen` 等），不触及 `get_available_screens`。所以"移除 tauri 才能测试"是伪需求。

**真实收益评估**：①编译——orb 仅被主 app 使用，主 app 必带 tauri，**零编译收益**；②测试——测试已存在，**零收益**。唯一剩余价值是"库 crate 不应依赖 GUI 框架"的架构洁癖。**结论：降级为可选（P3），不做也无实质损失。**

### 6.3 assistant-tools 的 publish 字段不一致

写的是 `publish = false`，其他 crate 用 `publish.workspace = true`。应统一。

### 6.4 build.rs 重复 ❌ 不建议提取

assistant-security/build.rs 和 assistant-storage/build.rs 做了相同的 Windows 系统库链接（crypt32 / ws2_32 / advapi32 / user32，各 4 行）。security 额外有 OPENSSL_DIR 处理（10 行，仅 security 需要）。

**实际重复仅 4 行**。提取公共 build 脚本需新建一个 build-support crate 并加为 build-dependency，引入的复杂度大于收益。且这 4 个 dylib 链接本质是 rusqlite/SQLCipher 原生库的链接需求，正确做法应是让 libsqlite3-sys 自行声明，而非人工重复。

**结论**：保持现状，或排查是否可完全移除（若 libsqlite3-sys 已自动链接）。不值得新建 crate。

### 6.5 缺少 feature flag 控制重依赖

image（agent）、zxcvbn（security）、keyring + 平台特定（security）、ssh-key + rsa（security）等依赖始终参与编译，应添加 feature gate 允许开发时跳过。

**已核实的依赖归属**（均只被单一模块使用，天然适合 feature gate）：
| 依赖 | 编译耗时 | 唯一使用者 | 可 gate |
|---|---|---|---|
| zxcvbn | 9.7s | `security/password_strength/mod.rs` | ✅ |
| arboard | 4.1s | `security/clipboard_guard.rs` | ✅ |
| ssh-key | 3.6s | `security/ssh_agent/validate.rs` | ✅ |
| rsa | 1.8s | security（ssh 相关） | ✅ |
| image | — | `agent`（图片处理） | ✅ |

**现实约束**：
1. 这些依赖在**生产构建中全部需要**，feature gate 只能加速**开发期**构建（`cargo check` 跳过 Vault/图片功能）。
2. 需配合 `#[cfg(feature = "...")]` 门控模块 + 提供 stub 实现，否则会编译失败。工作量非零。
3. 重依赖一旦编译完成即被缓存，**增量构建不会重复编译**。收益仅体现在 clean build（171.5s 中 zxcvbn 占 9.7s ≈ 5.7%）。

**建议**：若目标仅为缩短 CI 冷构建时间，可对 zxcvbn（9.7s，占比最大）单独做 feature gate；arboard/ssh-key/rsa 收益较小（合计 ~9.5s）且 stub 成本高，优先级低。

**更优替代**：直接执行 §6.1 拆分后，`assistant-vault`（含 crypto + sync + SQLCipher + keyring）与 `assistant-security`（含 zxcvbn/arboard/ssh-key/rsa/reqwest）各自持有重依赖，主 app 通过 feature 选择性启用，比在单个 security crate 内做 cfg 门控更清晰。**该方案依赖 §6.1 两步拆分落地（P2 必做项）**；若尚未拆分，本节的 feature gate 仍是唯一手段。

### 6.6 ~~rusqlite 双消费者~~ ❌ 不建议统一

assistant-security 和 assistant-storage 都依赖 rusqlite（bundled-sqlcipher），但使用**不同的数据库文件**：
- storage → `data_dir/assistant.db`（setup.rs L54）
- security → `data_dir/security/security.db`（setup.rs L96）

**不建议统一为 assistant-db crate**，原因：

1. **编译收益为零**：Cargo feature unification 机制下，workspace 中所有 rusqlite 消费者必然取 features 并集（`bundled-sqlcipher` ⊃ `bundled`），storage 无法真正只编译纯 SQLite。即使 storage 声明 `features = ["bundled"]`，最终解析仍为 `bundled + bundled-sqlcipher + modern_sqlite`，编译时间不变。
2. **Schema 无交集**：storage 的表（providers, conversations, messages, agent_modes, …）与 security 的表（vault_meta, vault_entries, security_audit_log）零共享，不存在"改了这边忘了那边"的风险。统一只是物理搬运，逻辑分散度不变。
3. **连接生命周期不同**：storage 用 `Mutex<Connection>`（启动时打开永不关闭），security 用 `Mutex<Option<Connection>>`（延迟打开，可 lock/unlock 反复），统一需引入 trait 抽象，过度设计。
4. **依赖边反而增加**：当前 2 条（storage→rusqlite, security→rusqlite），统一后 3 条（storage→db, security→db, db→rusqlite）。
5. **build.rs 重复仅 4 行**：不值得为此引入新 crate + feature gate 区分 OpenSSL 路径。

**保留现状**。仅当第三个 rusqlite 消费者出现时值得重新评估。

---

## 7. 目标架构

### 7.1 消除可消除的依赖边后

```
config (叶)    i18n (叶)    orb (叶)    tools (叶)    crash (叶)
providers (叶)        ← ✅ 已消除 security
storage (叶)          ← ✅ 已消除 i18n
agent   → providers, storage  ← ✅ 已消除 i18n
crypto  (叶)          ← 新建（§6.1）
sync    (叶)          ← vault_file + sync_engine 迁出后零内部依赖（§2.4）
vault   → crypto, sync        ← 新建（§6.1）
security → crypto             ← 缩小为 OS 交互层（§6.1）
protocol (叶)         ← ⚠️ 有条件（P3），见 §3.1.1
主 app  → 全部
```

### 7.1.1 拆分后的完整形态

```
crypto (叶)   ~700 行
  vault_crypto/{mod,types,totp} + crypto/api_key + error.rs(VaultError)
  MasterKey / PerEntryKey / CipherAlgorithm / VaultPlaintext / SecureString
  VaultCrypto / TOTP / API Key 加解密 / VaultError
  ⚠️ 不含 hibp（用 reqwest，移入 security）
  ⚠️ 不含 VaultEntry* DTO（属 vault 领域）

sync (叶)     ~1100 行
  git_sync/ + types/
  零内部 crate 依赖

vault → crypto, sync          ~5100 行
  security_db + types(VaultEntry* DTO) + recovery_key + audit
  + biometric + vault_file(自 sync 迁入) + sync_engine(自 sync 迁入)

security → crypto             ~2700 行
  unlock_dialog + screen_lock + ssh_agent + clipboard_guard
  + reveal_guard + password_strength + hibp(自 vault_crypto 迁入)
```

**关键点（相对上一版的 5 处修正）**：

1. **`sync` 变为纯叶**：`vault_file` 与 `sync_engine` 必须**同批**迁入 vault（§2.4），否则 `vault → sync`（用 vault_file）与 `sync → vault`（用 DTO）成环。
2. **`VaultError` 归 crypto 而非 vault**：它是 security 的共享错误枚举，`unlock_dialog`(9 处) / `ssh_agent`(4 处) / `reveal_guard`(2 处) / `clipboard_guard`(1 处) 都在用；归 vault 会让 security 反向依赖 vault。
3. **`types/` 拆两半**：`VaultError` → crypto；`VaultEntry*` / `VaultCategory` / `CustomFieldInput` / `ImportOutcome` / `VaultCryptoInfo` → vault。
4. **`biometric` 随 vault 走**：`security_db/biometric_ops.rs` 是其唯一调用方，留 security 会造成 `vault → security` 层次倒置。
5. **`hibp` 移入 security**：它是 `vault_crypto/` 下唯一使用 `reqwest` 的成员，留在 crypto 会让叶 crate 变成网络 crate。

**crate 规模变化**：

| Crate | 现状 | 拆分后 | 变化 |
|---|---|---|---|
| assistant-security | ~7000 | ~2700 | 缩小 |
| assistant-sync | ~2700 | ~1100 | 缩小 |
| assistant-crypto | — | ~700 | 新建 |
| assistant-vault | — | ~5100 | 新建 |

**新 crate 数量：2 个**（`assistant-crypto`、`assistant-vault`）。取消的独立 crate：`assistant-api-crypto`（并入 crypto）、`assistant-audit`（并入 vault）。

**收益（须限定场景）**：①`-p assistant-sync` 闭包缩减约 20s；②`-p assistant-vault` 可独立编译（无网络栈、无 OS-UI 栈）；③`-p assistant-security` 不再拉 SQLCipher/keyring；④**完整 workspace 构建零提速**。

**⚠️ 拆分对依赖边的真实影响**：内部边从 1 条增至 3 条，与 P0"减少边数"方向相反。拆分目的不是减少边数，而是**缩小每个 crate 的依赖闭包**——让轻消费者不再被迫拉入六类重依赖。主 app 闭包不变（本就依赖全部 crate）。

### 7.2 主 app 精简后

全部是 tauri 框架胶水代码（命令注册、状态管理、窗口管理、事件发射），不含领域逻辑。

---

## 8. 建议执行顺序

| 优先级 | 事项 | 风险 | 收益 |
|---|---|---|---|
| ~~P0~~ | ~~消除 storage → i18n、agent → i18n~~ ✅ 已完成 | — | 2 条边消除，storage/agent 变叶；结构化错误枚举 + 主 app 翻译桥接 |
| ~~P0~~ | ~~消除 providers → security~~ ✅ 已完成 | — | 1 条边消除，providers 变叶；`-p assistant-providers` 闭包减少 ~25s（全量构建零提速，见 §1.4） |
| P1→**P3** | 新建 assistant-protocol（**降级，有条件**，见 §3.1.1） | 低 | **核查发现：子 crate 对 DTO 引用 0 处；5 个 sync/security 类型直出前端绕过 DTO；前端 TS 为手写镜像且契约测试不校验 Rust** → 单抽 crate = 半截契约 + 零复用 + 零编译收益。**仅在同时补齐 5 个 DTO + 引入 ts-rs/typeshare 时才值得做**，否则留在主 app |
| ~~P1~~ | ~~新建 assistant-crash~~ ✅ 已完成 | — | 主 app 精简 87 行，crash 变叶 crate，可 feature gate |
| P1 | 存储分析 + 会话工具 → storage | 低（纯搬运，需剥离 tauri AppHandle 胶水） | 主 app 精简 197 行，领域逻辑归位 |
| ~~P1~~ | ~~Mascot 资产管理 → 新 crate assistant-mascot~~ ❌ 不建议（见 §4.3） | — | 命中检查表 ⑤：tauri command 胶水 + 单消费者 + 轻依赖 + 138 行；独立反把 tauri 拉入新叶 crate |
| P2 | Chat 消息插入 → agent | 低（前置：5 处 `assistant_i18n::t()` 改结构化错误；agent 已依赖 storage+providers） | 主 app 精简 258 行，storage 不变上帝 crate |
| P2 | **步 1：建 `assistant-crypto`（§6.1）** | 中（~60 处 import 机械替换；`VaultError` 从 `types/` 抽到 `error.rs`） | crypto 叶 crate ~700 行，服务 vault / security / 主 app；`-p assistant-crypto` 闭包无 rusqlite/reqwest/arboard/zxcvbn/ssh-key |
| P2 | **步 2：建 `assistant-vault`（§6.1 + §2.4）** | **高**（~5100 行迁移；`vault_file` + `sync_engine` **必须同批**迁入，否则成环；23 个签名 + 主 app 4 文件调用点） | 真正消除 `sync → security`；`-p assistant-vault` 独立编译（无网络栈/OS-UI 栈）；`-p assistant-sync` 闭包 -20s |
| P3 | ~~workspace.dependencies 精简（§5.2）~~ ⚠️ 降级 | 低 | 仅根文件可读性，有 churn 风险；**建议不做**，拆分副产品自然归位即可 |
| P3 | orb 去耦合 tauri（§6.2） | 低（仅 placement.rs 的 `get_available_screens` 一个函数耦合） | 仅架构洁癖；**编译/测试均零收益**（测试已存在） |
| ~~P3~~ | ~~build.rs 去重（§6.4）~~ ❌ 不建议 | — | 重复仅 4 行，提取需新建 crate，得不偿失 |
| P3 | feature flag 添加（§6.5） | 低（仅建议对 zxcvbn 9.7s 做，其余 stub 成本高） | 开发/CI 冷构建加速 |

### 8.1 关键依赖关系（执行顺序约束）

```
§6.1 步1（建 assistant-crypto：vault_crypto/{mod,types,totp} + crypto/api_key + VaultError）
   └──> 步2（建 assistant-vault：security_db + types(VaultEntry* DTO) + recovery_key + audit
             + biometric + vault_file + sync_engine【两者必须同批】）
        ══> §2.4 sync→security 彻底消除（sync 收缩为纯叶 crate）
   └──> 主 app 改为 assistant-vault::setup() 单入口（前端懒加载已完成）
   └──> 依赖自然落到新 crate（不必单独做 workspace 精简，见 §5.2）
```

§4.1 protocol 断开 4 处类型引用 ──> §3.1.1 补齐 5 个直出类型 DTO + TS 代码生成 ──> §3.1 assistant-protocol 创建

§5.3 windows feature 精确化（独立项，与 orb 无关；P3 可选）
§6.2 orb 去 tauri（独立项；P3 可选）
```

**已降级/否决项汇总**：
- §3.1 assistant-protocol → ⚠️ 降级 P3（子 crate 引用 0 处、5 个类型直出前端、前端 TS 手写镜像；单抽 crate 无收益，须先补齐契约 + TS 代码生成）
- §2.3 providers→security → ✅ 已用「凭证注入」解决，**优于原文档方案 A/B**（providers 无需加密能力，加密归 security，零新 crate、零重复代码）
- §6.4 build.rs 去重 → ❌ 否决（4 行重复）
- §6.6 rusqlite 统一 → ❌ 否决（feature unification 零收益）
- §6.1 的 api-crypto(api_key) 拆分 → ❌ 取消独立 crate，**并入 `assistant-crypto`**（与 vault 加密原语同属全应用加密函数）
- §6.1 **audit 独立成 crate** → ❌ 否决（92 行 < 300、仅 rusqlite 依赖、主 app 与 vault 已共享，拆出零收益），并入 vault 内部模块
- §6.1 **vault 独立成 crate** → ✅ **必做（P2）**。原判"降级 P3 可选、仅主 app 消费"是错的：`sync_engine` 实测 23 处 `&SecurityDbState` 签名、17 处 db 调用，不迁 vault 则 `sync → security` 无法真正消除
- §6.1 **crypto crate 收容范围** → ⚠️ 必须排除 `vault_crypto/hibp.rs`（用 reqwest，移入 security）；**`VaultError` 归 crypto**（security 4 模块共用，归 vault 会造成反向依赖）；**`VaultEntry*` 等 DTO 归 vault**
- §6.1 **sync_engine / vault_file 归属** → ⚠️ **必须同批迁入 vault**；若 `vault_file` 留 sync 而 `sync_engine` 迁 vault，`vault → sync → vault` 成环
- §6.1 **biometric 归属** → ⚠️ 随 vault 走，不能留在 security（否则 `vault → security` 数据层反向依赖 OS 层）
- §6.1 **命名** → ✅ 采用 `assistant-crypto`（全应用级加密函数：Argon2/HKDF/AES-GCM/ChaCha20/TOTP/API Key），取代上一轮的 `assistant-vault-crypto` / `assistant-api-crypto` 二分
- §5.2 workspace.dependencies 激进精简 → ⚠️ 降级（版本单点管理是核心价值，与消费者数无关；移出有 churn 风险）
- §5.3 windows feature 精确化 → ⚠️ 降级 P3（**机制更正**：Cargo 支持 feature 叠加、不支持削减；修法是 workspace 只留版本、成员按需声明。收益仅 `-p assistant-tools`，全量构建因 unification 零提速）
- §6.1 拆分命名 → ✅ 最终采用 `assistant-crypto` 单 crate（含 vault 加密原语 + API Key 加解密），不再二分
- §6.2 orb 去 tauri → ⚠️ 降级 P3（测试已存在，编译零收益；仅架构洁癖）
- §3.2 API Key 掩码 → providers 搬运建议过时（providers 已无 crypto），且主 app 已依赖 security，搬运无收益
- §3.2 Mascot 资产管理 → ❌ 否决独立 crate（命中 §4.3 检查表 ⑤：`#[command]` + `AppHandle` 胶水、单消费者、轻依赖、138 行；独立反将 tauri 拉入新叶节点），改为留在主 app
- §3.2 / §4.1 Vault 编排 → security + `VaultConfig` trait → ❌ 撤回（security 现为零内部依赖的纯叶节点；搬入需再依赖 config+tauri 并用 trait"化解"，与 §2.4 否定的 trait 注入同一反模式；编排留在主 app）
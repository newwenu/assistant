# I9 — Vault 职责拆分规格

> 生成日期：2026-10-22
> 关联文档：[I9-vault-spec.md](./I9-vault-spec.md)、[I9-vault-issues.md](./I9-vault-issues.md)、[I9-vault-deadcode.md](./I9-vault-deadcode.md)
> 参考先例：[I19-code-responsibility.md](../achievement1/I19-code-responsibility.md)（chatStore/settingsDispatcher 拆分模式）
> 目标：将 vault 模块 3 个巨型文件 + 4 个偏大组件拆分为职责单一的小模块，同时保持外部 API 不变

---

## 1 问题总览

| # | 文件 | 当前行数 | 严重度 | 核心问题 |
|---|------|---------|--------|---------|
| 1 | `src-tauri/crates/assistant-security/src/security_db/mod.rs` | **1,579** | 🔴 严重 | DB 状态 + CRUD + 加解密 + 生物识别 + 密钥轮换全塞一个 impl |
| 2 | `src-tauri/src/cmd/vault.rs` | **1,241** | 🔴 严重 | 30+ 个 Tauri command + 辅助函数全塞一个文件 |
| 3 | `src/vault/core/vaultStore.ts` | **884** | 🔴 严重 | 30+ state + 40+ action 全塞一个 Zustand store |
| 4 | `src/vault/core/vault.service.ts` | **313** | 🟡 偏大 | 15+ 类型定义 + 40+ RPC 封装混放 |
| 5 | `src/vault/VaultUnlock.tsx` | **523** | 🟡 偏大 | 密码/生物识别/恢复密钥三种解锁方式未拆子组件 |
| 6 | `src/vault/VaultEntryDrawer.tsx` | **523** | 🟡 偏大 | 6 种 category 表单未拆子组件 |
| 7 | `src/vault/detail/VaultEntryDetail.tsx` | **433** | 🟡 偏大 | TOTP/HIBP/揭示逻辑可抽 hook |
| 8 | `src/vault/settings/VaultSettings.tsx` | **335** | 🟡 偏大 | 6 个设置域可拆子组件 |

---

## 2 功能域划分

根据 vault 的业务语义，划分为 **7 个功能域**：

| 域 | 代号 | 职责 | 涉及 spec |
|---|------|------|----------|
| 生命周期 | `lifecycle` | init / unlock / lock / checkStatus / autoLock | spec-1 §3, spec-3 §4 |
| 条目 CRUD | `entries` | listEntries / getEntry / createEntry / updateEntry / deleteEntry / search / filter | spec-2 §2 |
| 字段操作 | `reveal` | copyPassword / copyField / revealPassword / revealNotes / clearRevealedField | spec-2 §2.3, spec-3 §2 |
| TOTP + HIBP | `audit` | generateTotp / copyTotp / hibpCheck / hibpAudit / checkMasterPasswordStrength | spec-3 §5, §6 |
| 生物识别 | `biometric` | checkAvailable / checkEnrolled / unlockBiometric / enrollBiometric / disenrollBiometric | spec-3 §8, spec-3a |
| 同步 + SSH | `sync` | syncPull/Push/Now/ForcePush / gitInit/Clone/AddRemote/SetRemote / resolveConflict / sshAdd/Remove/List/AutoAdd/CheckRunning | spec-4 §11, spec-5 §3 |
| 恢复 + 完整性 | `recovery` | generateRecoveryKey / unlockWithRecoveryKey / verifyIntegrity / disasterRecovery* / exportVault / importVault | spec-5 §6, §12, §13 |

---

## 3 Rust 侧拆分方案

### 3.1 `security_db/mod.rs`（1,579 行 → 7 文件）

#### 目标结构

```
security_db/
├── mod.rs              ← SecurityDbState struct 定义 + 生命周期方法（~250 行）
├── crud.rs             ← list_entries / get_entry / create_entry / update_entry / delete_entry（~250 行）
├── crypto_ops.rs       ← decrypt_password / decrypt_notes / decrypt_totp_secret / decrypt_field / decrypt_ssh_private_key / decrypt_entry（~200 行）
├── rekey.rs            ← change_master_password（含全量重加密事务）（~200 行）
├── biometric_ops.rs    ← enroll_biometric / unlock_biometric / disenroll_biometric / is_biometric_enrolled / check_biometric_consistency（~350 行）
├── import_export.rs    ← export_entries / import_entries（~150 行）
├── schema.rs           ← DDL（不变，~50 行）
└── meta.rs             ← vault_meta KV 读写（不变，~98 行）
```

#### 各文件职责

| 文件 | pub 方法 | 依赖 | 行数估算 |
|------|---------|------|---------|
| `mod.rs` | `new`, `init`, `unlock`, `lock`, `is_locked`, `get_conn`, `get_master_key`, `get_bootstrap_key`, `backup`, `is_initialized`, `cipher_algorithm_from_conf`, `db_path`, `set_conn_and_key`, `with_conn_and_key` | `vault_crypto`, `schema`, `meta`, `audit` | ~250 |
| `crud.rs` | `list_entries`, `get_entry`, `create_entry`, `update_entry`, `delete_entry` | `mod.rs`(with_conn_and_key), `vault_crypto`, `audit`, `types` | ~250 |
| `crypto_ops.rs` | `decrypt_password`, `decrypt_notes`, `decrypt_totp_secret`, `decrypt_field`, `decrypt_ssh_private_key`, `decrypt_entry` | `mod.rs`(with_conn_and_key), `vault_crypto`, `types` | ~200 |
| `rekey.rs` | `change_master_password` | `mod.rs`, `vault_crypto`, `schema`, `meta`, `audit`, `biometric_ops`(disenroll) | ~200 |
| `biometric_ops.rs` | `biometric_key_path`, `is_biometric_available`, `enroll_biometric`, `unlock_biometric`, `disenroll_biometric`, `is_biometric_enrolled`, `check_biometric_consistency` | `mod.rs`, `vault_crypto`, `biometric::BiometricKeyStore`, `audit` | ~350 |
| `import_export.rs` | `export_entries`, `import_entries` | `mod.rs`, `crypto_ops`(decrypt_entry), `types` | ~150 |

#### 关键设计决策

1. **`SecurityDbState` struct 定义留在 `mod.rs`**，子模块通过 `impl SecurityDbState` 扩展（Rust 允许跨文件 impl 同一 struct，只要在同一个 crate 内）
2. **`with_conn_and_key` 留在 `mod.rs`** 作为子模块共享的"获取 conn + master_key"辅助方法
3. **`rekey.rs` 依赖 `biometric_ops::disenroll_biometric`**：改密后需 disenroll 生物识别。`rekey.rs` 调用 `self.disenroll_biometric()`，由于都在同一 impl 块，Rust 编译允许
4. **条件编译 `#[cfg(target_os)]` 集中在 `biometric_ops.rs`**，不再污染主文件
5. **`custom_fields_eq` 辅助函数移入 `import_export.rs`**，仅该处使用

#### 迁移步骤

1. 创建 `security_db/crud.rs`，从 `mod.rs` 剪切 `list_entries` / `get_entry` / `create_entry` / `update_entry` / `delete_entry` 及 `custom_fields_eq`
2. 创建 `security_db/crypto_ops.rs`，剪切所有 `decrypt_*` 方法
3. 创建 `security_db/rekey.rs`，剪切 `change_master_password`
4. 创建 `security_db/biometric_ops.rs`，剪切所有 `*_biometric*` 方法
5. 创建 `security_db/import_export.rs`，剪切 `export_entries` / `import_entries`
6. `mod.rs` 添加 `mod crud; mod crypto_ops; mod rekey; mod biometric_ops; mod import_export;`，保留 struct 定义 + 生命周期方法
7. 编译验证 + 运行 `vault_crypto` / `vault_crud` / `security_db` 测试

---

### 3.2 `cmd/vault.rs`（1,241 行 → 8 文件）

#### 目标结构

```
cmd/vault/
├── mod.rs              ← pub(crate) 辅助函数 + re-export 所有 command（~100 行）
├── lifecycle.rs        ← vault_init / vault_unlock / vault_is_locked / vault_is_initialized / vault_lock / vault_show_unlock_window（~120 行）
├── entries.rs          ← vault_list_entries / vault_get_entry / vault_create_entry / vault_update_entry / vault_delete_entry / vault_change_master_password / vault_export / vault_import（~200 行）
├── reveal.rs           ← vault_copy_password / vault_copy_field / vault_reveal_password / vault_reveal_notes（~120 行）
├── audit.rs            ← vault_totp_* / vault_query_audit_log / vault_hibp_* / vault_check_master_password_strength（~120 行）
├── biometric.rs        ← vault_is_biometric_available / vault_is_biometric_enrolled / vault_unlock_biometric / vault_enroll_biometric / vault_disenroll_biometric（~80 行）
├── sync.rs             ← vault_sync_* / vault_git_* / vault_resolve_conflict（~120 行）
├── ssh.rs              ← vault_ssh_*（~120 行）
└── recovery.rs         ← vault_generate_recovery_key / vault_unlock_with_recovery_key / vault_has_recovery_key / vault_verify_vault_meta_integrity / vault_disaster_recovery_export（~120 行）
```

#### 各文件职责

| 文件 | `#[command]` 函数 | 依赖 | 行数估算 |
|------|-------------------|------|---------|
| `mod.rs` | 无 command | `ssh_bridge_from_conf`, `ssh_remove_all_on_lock`, `ssh_auto_add_on_unlock`, `vault_state`, `cipher_algo`, `reveal_guard`, `clipboard_guard`, `get_clipboard_clear_secs`, `sync_state`, `get_key_store` | ~100 |
| `lifecycle.rs` | 6 个 | `mod.rs` 辅助, `security_db`, `audit`, `ssh`(auto_add) | ~120 |
| `entries.rs` | 8 个 | `mod.rs` 辅助, `security_db`, `audit`, `clipboard_guard` | ~200 |
| `reveal.rs` | 4 个 | `mod.rs` 辅助, `security_db`, `reveal_guard`, `audit` | ~120 |
| `audit.rs` | 7 个 | `mod.rs` 辅助, `security_db`, `vault_crypto::totp`, `vault_crypto::hibp`, `password_strength` | ~120 |
| `biometric.rs` | 5 个 | `mod.rs` 辅助, `security_db`, `ssh`(auto_add) | ~80 |
| `sync.rs` | 11 个 | `mod.rs` 辅助, `assistant_sync::SyncEngine` | ~120 |
| `ssh.rs` | 5 个 | `mod.rs` 辅助, `security_db`, `ssh_agent`, `audit` | ~120 |
| `recovery.rs` | 5 个 | `mod.rs` 辅助, `security_db`, `assistant_sync`, `recovery_key` | ~120 |

#### 关键设计决策

1. **辅助函数留在 `mod.rs`**，子模块通过 `super::` 引用（如 `super::vault_state(&app)?`）
2. **`ssh_remove_all_on_lock` 和 `ssh_auto_add_on_unlock` 留在 `mod.rs`**，因为被 `lifecycle.rs` 和 `biometric.rs` 共同依赖
3. **`DisasterRecoveryResult` struct 移入 `recovery.rs`**，仅该处使用
4. **`REVEAL_TTL_SECS` 常量移入 `reveal.rs`**，仅该处使用
5. **Tauri command 注册处**需要更新 `invoke_handler` 中的 `vault::*` 引用方式

#### 迁移步骤

1. 将 `cmd/vault.rs` 重命名为 `cmd/vault/mod.rs`
2. 依次创建子文件，按功能域剪切对应 `#[command]` 函数
3. `mod.rs` 保留辅助函数 + `mod` 声明 + `pub use` re-export
4. 更新 `src-tauri/src/cmd/mod.rs` 中的 `pub mod vault;`（无需改动，因为 vault 现在是目录）
5. 更新 `invoke_handler` 注册（如果使用 `vault::*` 通配则无需改动）
6. 编译验证

---

## 4 前端侧拆分方案

### 4.1 `vaultStore.ts`（884 行 → 1 组装 + 7 slice）

采用 Zustand slice pattern（与 `chatStore` 拆分先例一致）。

#### 目标结构

```
vault/core/
├── vaultStore.ts              ← slice 组装 + re-export（~80 行）
├── vault.types.ts             ← 所有 TS 类型定义（~80 行）
├── vault.service.ts           ← RPC 封装（不变，~313 行，但 import 类型从 vault.types.ts）
├── useVaultEvents.ts          ← 不变（33 行）
├── useVaultAutoLock.ts        ← 不变（25 行）
├── slices/
│   ├── vaultLifecycleSlice.ts ← isLocked / isInitialized / loading / error + init / unlock / lock / checkStatus / clearError / resetAutoLockTimer（~120 行）
│   ├── vaultEntriesSlice.ts   ← entries / selectedEntry / searchQuery / categoryFilter / tagFilter + loadEntries / selectEntry / clearSelectedEntry / createEntry / updateEntry / deleteEntry / setSearchQuery / setCategoryFilter / setTagFilter（~150 行）
│   ├── vaultRevealSlice.ts    ← revealedField + revealPassword / revealNotes / clearRevealedField / copyPassword / copyField（~100 行）
│   ├── vaultAuditSlice.ts     ← totpCode / totpTimeRemaining / totpTimerId / hibpLeaks / hibpAuditing + generateTotp / copyTotp / stopTotpTimer / totpParseUri / hibpCheck / hibpAudit / checkMasterPasswordStrength / queryAuditLog / showUnlockWindow（~150 行）
│   ├── vaultBiometricSlice.ts ← biometricAvailable / biometricEnrolled / _biometricAvailableAt / _biometricEnrolledAt + checkBiometricAvailable / checkBiometricEnrolled / unlockBiometric / enrollBiometric / disenrollBiometric（~100 行）
│   ├── vaultSyncSlice.ts      ← syncReadyness / syncResult / syncConfig / syncing / sshAgentKeys / sshAgentRunning + sync* / git* / resolveConflict / ssh*（~200 行）
│   └── vaultRecoverySlice.ts  ← recoveryKeySet / _recoveryKeySetAt / integrityResult / disasterRecoveryStep / disasterRecoveryResult / exportFilePath + generateRecoveryKey / unlockWithRecoveryKey / checkRecoveryKeySet / verifyIntegrity / disasterRecovery* / exportVault / importVault（~150 行）
```

#### Slice 接口定义

每个 slice 导出一个 `VaultXxxSlice` 接口和一个 `createXxxSlice` 工厂函数：

```ts
// slices/vaultLifecycleSlice.ts
export interface VaultLifecycleSlice {
  isLocked: boolean;
  isInitialized: boolean;
  loading: boolean;
  error: string | null;
  autoLockTimerId: ReturnType<typeof setTimeout> | null;

  init: (masterPassword: string) => Promise<void>;
  unlock: (masterPassword: string) => Promise<boolean>;
  lock: () => Promise<void>;
  checkStatus: () => Promise<void>;
  clearError: () => void;
  resetAutoLockTimer: () => void;
}

type FullState = VaultLifecycleSlice & { /* 跨 slice 字段 */ };

export function createLifecycleSlice(
  set: (partial: Partial<FullState> | ((state: FullState) => Partial<FullState>)) => void,
  get: () => FullState,
): VaultLifecycleSlice;
```

#### 组装方式

```ts
// vaultStore.ts
import { create } from "zustand";
import { createLifecycleSlice } from "./slices/vaultLifecycleSlice";
import { createEntriesSlice } from "./slices/vaultEntriesSlice";
// ... 其余 slice

export type VaultState = VaultLifecycleSlice & VaultEntriesSlice & VaultRevealSlice
  & VaultAuditSlice & VaultBiometricSlice & VaultSyncSlice & VaultRecoverySlice;

export const useVaultStore = create<VaultState>()((set, get): VaultState => {
  const lifecycleSlice = createLifecycleSlice(set, get);
  const entriesSlice = createEntriesSlice(set, get);
  const revealSlice = createRevealSlice(set, get);
  const auditSlice = createAuditSlice(set, get);
  const biometricSlice = createBiometricSlice(set, get);
  const syncSlice = createSyncSlice(set, get);
  const recoverySlice = createRecoverySlice(set, get);
  return {
    ...lifecycleSlice,
    ...entriesSlice,
    ...revealSlice,
    ...auditSlice,
    ...biometricSlice,
    ...syncSlice,
    ...recoverySlice,
  };
});
```

#### 关键设计决策

1. **`syncSlice` 合并同步 + SSH**：SSH Agent 操作与同步共享 `sshAgentKeys` / `sshAgentRunning` 状态，且 SSH 依赖同步引擎的 `sync_state`，拆开反而增加跨 slice 依赖
2. **`vaultService` 单例由各 slice 自行 import**，不通过参数注入（与当前模式一致）
3. **`useSettingsStore` 仅 `lifecycleSlice` 引用**（读 `vaultAutoLockMinutes`），其他 slice 不依赖设置
4. **`BIOMETRIC_CACHE_TTL_MS` 常量移入 `vaultBiometricSlice.ts`**
5. **`DEFAULT_AUTO_LOCK_MINUTES` + `getAutoLockMs()` 积入 `vaultLifecycleSlice.ts`**
6. **原 `vaultStore.ts` 保留 re-export**：`export { useVaultStore } from "./vaultStore"` 确保外部 import 路径不变
7. **slice 文件放 `slices/` 子目录**：7 个 slice 构成强内聚域，与 `settings/core/field/`、`settings/core/provider/` 的子目录组织先例一致；`core/` 根层仅保留组装、类型、服务、hook、测试 5 个文件

#### 迁移步骤

1. 创建 `vault/core/vault.types.ts`，从 `vault.service.ts` 剪切所有 `export type` / `export interface`
2. `vault.service.ts` 改为 `import type { ... } from "./vault.types"`
3. 创建 `vault/core/slices/` 目录
4. 依次创建 7 个 slice 文件（`vaultLifecycleSlice.ts` 等），从 `vaultStore.ts` 剪切对应 state + action
5. 重写 `vaultStore.ts` 为 slice 组装（`create<VaultState>()((set, get) => { ... })` 模式）
6. 更新 `vaultStore.test.ts` 的 import（如果直接引用了 state 字段）
7. 运行测试验证

---

### 4.2 `vault.service.ts` 类型提取（313 行 → 80 行类型 + 233 行 RPC）

#### 目标

```
vault/core/
├── vault.types.ts     ← 15+ interface/type 定义（~80 行）
└── vault.service.ts   ← vaultService 对象，仅 RPC 封装（~233 行）
```

#### `vault.types.ts` 包含的类型

```ts
export type VaultCategory = "login" | "note" | "card" | "identity" | "sshKey" | "custom";
export interface VaultEntrySummary { ... }
export interface VaultEntryDetail { ... }
export interface RevealResult { ... }
export interface CustomFieldInput { ... }
export interface VaultEntryCreate { ... }
export interface VaultEntryUpdate { ... }
export interface TotpResult { ... }
export interface ParsedOtpAuth { ... }
export interface AuditLogEntry { ... }
export interface StrengthResult { ... }
export type Readyness = ...;
export type RepositoryState = ...;
export type SyncResult = ...;
export interface ConflictFile { ... }
export interface SyncConfig { ... }
export type ConflictResolution = ...;
export interface SshKeyInfo { ... }
export interface IntegrityResult { ... }
export interface DisasterRecoveryResult { ... }
```

#### `vault.service.ts` 变更

- 删除所有 `export type` / `export interface`
- 添加 `import type { ... } from "./vault.types"`
- `vaultStore` 和 UI 组件改为 `import type { ... } from "./vault.types"` 或从 `vaultStore` re-export

---

### 4.3 UI 组件拆分

#### 4.3.1 `VaultUnlock.tsx`（523 行 → 1 容器 + 3 子组件 + 1 hook）

```
vault/
├── VaultUnlock.tsx              ← 容器：tab 切换 + 状态编排（~80 行）
├── unlock/
│   ├── PasswordForm.tsx         ← 主密码输入 + 提交（~100 行）
│   ├── BiometricButton.tsx      ← 生物识别按钮 + 状态（~80 行）
│   └── RecoveryKeyForm.tsx      ← 恢复密钥输入 + 提交（~100 行）
└── core/
    └── useVaultUnlock.ts        ← 解锁逻辑 hook（init/unlock/unlockBiometric/unlockWithRecoveryKey + loading/error）（~80 行）
```

#### 4.3.2 `VaultEntryDrawer.tsx`（523 行 → 1 容器 + 6 表单子组件）

```
vault/
├── VaultEntryDrawer.tsx         ← 容器：Drawer + category 选择 + 通用字段（~120 行）
└── forms/
    ├── LoginFields.tsx           ← username + url + password 字段（~60 行）
    ├── NoteFields.tsx            ← notes 字段（~40 行）
    ├── CardFields.tsx            ← cardholder + number + expiry + cvv（~80 行）
    ├── IdentityFields.tsx        ← identity 字段（~60 行）
    ├── SshKeyFields.tsx          ← private key + public key 字段（~60 行）
    └── CustomFields.tsx          ← custom fields 动态表单（~80 行）
```

#### 4.3.3 `VaultEntryDetail.tsx`（433 行 → 1 容器 + 3 子组件 + 1 hook）

```
vault/detail/
├── VaultEntryDetail.tsx         ← 容器：Descriptions + 操作按钮（~120 行）
├── TotpDisplay.tsx              ← TOTP 倒计时圆环 + 复制（~80 行，已存在内联组件，提取为独立文件）
├── HibpBadge.tsx                ← HIBP 泄露标记（~30 行）
├── RevealFieldButton.tsx        ← 揭示密码/笔记按钮（~50 行）
└── useEntryDetail.ts            ← selectEntry + reveal + copy + totp + hibp 逻辑 hook（~100 行）
```

#### 4.3.4 `VaultSettings.tsx`（335 行 → 1 容器 + 5 子组件）

```
vault/settings/
├── VaultSettings.tsx            ← Tabs 容器（~60 行）
├── VaultSyncSettings.tsx        ← 不变（253 行）
├── VaultConflictResolve.tsx     ← 不变（81 行）
├── SshAgentSettings.tsx         ← SSH Agent 管理 tab（~80 行）
├── RecoveryKeySettings.tsx      ← 恢复密钥 tab（~80 行）
├── IntegritySettings.tsx        ← 完整性验证 + 灾难恢复 tab（~80 行）
└── DangerZoneConfirm.tsx        ← 不变（84 行）
```

---

## 5 拆分优先级与执行顺序

### 阶段 A：Rust 侧核心拆分（影响最大，编译收益最高）

| 步骤 | 操作 | 预期收益 |
|------|------|---------|
| A1 | `security_db/mod.rs` → 7 文件 | 单文件 1579→250 行，改 CRUD 不重编译 biometric |
| A2 | `cmd/vault.rs` → 8 文件 | 单文件 1241→100 行，改 sync 不重编译 entries |
| A3 | 编译 + 运行 `vault_crypto` / `vault_crud` / `security_db` 测试 | 验证无回归 |

### 阶段 B：前端 Store 拆分（渲染性能收益）

| 步骤 | 操作 | 预期收益 |
|------|------|---------|
| B1 | 提取 `vault.types.ts` | 类型与服务解耦 |
| B2 | `vaultStore.ts` → 7 slice + 组装 | 884→80 行主文件，组件可按域订阅 |
| B3 | 更新 `vaultStore.test.ts` | 测试按 slice 隔离 |
| B4 | 运行前端测试 | 验证无回归 |

### 阶段 C：UI 组件拆分（可维护性收益）

| 步骤 | 操作 | 预期收益 |
|------|------|---------|
| C1 | `VaultUnlock.tsx` → 容器 + 3 子组件 + hook | 523→80 行容器 |
| C2 | `VaultEntryDrawer.tsx` → 容器 + 6 表单 | 523→120 行容器 |
| C3 | `VaultEntryDetail.tsx` → 容器 + 3 子组件 + hook | 433→120 行容器 |
| C4 | `VaultSettings.tsx` → Tabs 容器 + 3 子组件 | 335→60 行容器 |

---

## 6 风险与约束

### 6.1 Rust 跨文件 impl

Rust 允许在同一 crate 内跨文件为同一 struct 写 `impl` 块，但 **trait impl 必须在定义 struct 的文件中或定义 trait 的文件中**。当前 `SecurityDbState` 没有实现外部 trait，仅 `impl SecurityDbState { ... }`，因此跨文件拆分安全。

### 6.2 Zustand slice 类型安全

Slice 工厂函数的 `set` 参数类型为 `Partial<FullState> | ((state: FullState) => Partial<FullState>)`，需要确保每个 slice 的 `set` 调用只修改自己域的字段。通过 TypeScript 的 `Partial<FullState>` 类型约束实现编译期检查。

### 6.3 外部 API 不变

- `vaultStore.ts` 保留 `export { useVaultStore }` re-export，所有 `import { useVaultStore } from "./core/vaultStore"` 无需修改
- `vault.service.ts` 保留 `export const vaultService`，类型改为从 `vault.types.ts` re-export
- Rust 侧 `pub` 方法签名不变，仅文件组织变化

### 6.4 测试覆盖

- Rust：现有 `vault_crypto.rs` / `vault_crud.rs` / `security_db.rs` 测试应全部通过
- 前端：`vaultStore.test.ts` 需更新 import 路径，测试逻辑不变

---

## 7 拆分后行数估算

### Rust 侧

| 文件 | 当前行数 | 拆分后行数 |
|------|---------|-----------|
| `security_db/mod.rs` | 1,579 | ~250 |
| `security_db/crud.rs` | — | ~250 |
| `security_db/crypto_ops.rs` | — | ~200 |
| `security_db/rekey.rs` | — | ~200 |
| `security_db/biometric_ops.rs` | — | ~350 |
| `security_db/import_export.rs` | — | ~150 |
| `cmd/vault/mod.rs` | 1,241 | ~100 |
| `cmd/vault/lifecycle.rs` | — | ~120 |
| `cmd/vault/entries.rs` | — | ~200 |
| `cmd/vault/reveal.rs` | — | ~120 |
| `cmd/vault/audit.rs` | — | ~120 |
| `cmd/vault/biometric.rs` | — | ~80 |
| `cmd/vault/sync.rs` | — | ~120 |
| `cmd/vault/ssh.rs` | — | ~120 |
| `cmd/vault/recovery.rs` | — | ~120 |

### 前端侧

| 文件 | 当前行数 | 拆分后行数 |
|------|---------|-----------|
| `vaultStore.ts` | 884 | ~80 |
| `vault.types.ts` | — | ~80 |
| `slices/lifecycleSlice.ts` | — | ~120 |
| `slices/entriesSlice.ts` | — | ~150 |
| `slices/revealSlice.ts` | — | ~100 |
| `slices/auditSlice.ts` | — | ~150 |
| `slices/biometricSlice.ts` | — | ~100 |
| `slices/syncSlice.ts` | — | ~200 |
| `slices/recoverySlice.ts` | — | ~150 |
| `VaultUnlock.tsx` | 523 | ~80 |
| `VaultEntryDrawer.tsx` | 523 | ~120 |
| `VaultEntryDetail.tsx` | 433 | ~120 |
| `VaultSettings.tsx` | 335 | ~60 |

### 最大单文件行数变化

| 文件 | 当前行数 | 拆分后最大行数 | 降幅 |
|------|---------|--------------|------|
| `security_db/mod.rs` | 1,579 | 350（biometric_ops） | -78% |
| `cmd/vault.rs` | 1,241 | 200（entries） | -84% |
| `vaultStore.ts` | 884 | 200（syncSlice） | -77% |

---

## 8 与已有文档的关系

| 文档 | 关系 |
|------|------|
| [I9-vault-spec-1~5](./I9-vault-spec-1-foundation.md) | 本文档的拆分方案不改变 spec 定义的功能和 API，仅改变代码组织 |
| [I9-vault-issues.md](./I9-vault-issues.md) | 问题 1（SSH 锁定移除）和问题 2（SSH 解锁自动添加）已在 `cmd/vault/ssh.rs` 和 `cmd/vault/lifecycle.rs` 中修复，拆分后代码位置更清晰 |
| [I9-vault-deadcode.md](./I9-vault-deadcode.md) | 死代码清理已完成，拆分不会引入新的死代码 |
| [I19-code-responsibility.md](../achievement1/I19-code-responsibility.md) | 本文档沿用 I19 的 Zustand slice pattern 和"保留 re-export"策略 |

---

## 9 项目风格对齐审查

> 以下逐项对比项目现有约定，检查拆分方案是否与之对齐，标注偏差及修正。

### 9.1 Rust crate 内模块组织风格

**项目约定**（观察 `assistant-security`、`assistant-providers`、`assistant-sync`、`assistant-storage` 四个 crate）：

| crate | `lib.rs` 风格 | 子模块组织 |
|-------|--------------|-----------|
| `assistant-security` | `pub mod` 平铺 | 子模块自身再按需建目录（如 `biometric/`、`vault_crypto/`、`ssh_agent/`） |
| `assistant-providers` | `pub mod` 平铺 | 每个 provider 一个目录（`anthropic/`、`openai/`、`gemini/`），内部 `mod.rs` + 拆分文件 |
| `assistant-storage` | `pub mod` 平铺 | 每个表一个 `.rs` 文件，无目录 |
| `assistant-sync` | `pub mod` 平铺 + `pub use types::*` | 每个域一个目录（`git_sync/`、`sync_engine/`、`vault_file/`） |

**结论**：项目既有"每表一文件"（`assistant-storage`），也有"每域一目录"（`assistant-providers`、`assistant-sync`）。`security_db/` 当前只有 `mod.rs` + `meta.rs` + `schema.rs` 三个文件，拆分为 7 个文件符合"每功能域一文件"的 `assistant-storage` 风格，**无需再建子目录**。

**⚠️ 偏差 1**：文档中 `security_db/` 目标结构写的是 7 个 `.rs` 文件平铺，这 **符合项目风格**。但需确认 `lib.rs` 不需要改动——当前 `lib.rs` 只有 `pub mod security_db;`，拆分后 `security_db/mod.rs` 自行声明子模块，`lib.rs` 无需变更。✅ 已对齐。

### 9.2 Rust `cmd/` 目录组织风格

**项目约定**（观察 `src-tauri/src/cmd/`）：

- 所有 command 模块都是 **单文件**（`chat.rs`、`config.rs`、`conversation.rs` 等）
- `mod.rs` 用 `pub mod xxx;` + `pub use xxx::*;` 逐个 re-export
- **没有任何 cmd 子目录先例**

**⚠️ 偏差 2**：将 `vault.rs` 改为 `vault/` 目录是 **cmd 下的首个目录型模块**。这打破了"cmd 下全是单文件"的约定。

**修正方案**：两种选择——

| 方案 | 优点 | 缺点 |
|------|------|------|
| A. `cmd/vault/` 目录（文档当前方案） | vault 有 55 个 command，单文件 1241 行已不可维护；目录模式在 `assistant-providers` 等 crate 内有先例 | cmd 层首次出现目录，与其他 cmd 模块不一致 |
| B. 保持 `cmd/vault.rs` 单文件，仅将辅助函数抽到 `cmd/vault_utils.rs` | 保持 cmd 层风格一致 | 1241 行仅减少 ~100 行，收益极低 |

**决策**：采用 **方案 A**。理由：vault 是 cmd 层唯一超过 500 行的模块（次大是 `chat.rs` 约 400 行），其他模块不需要目录拆分。vault 的 55 个 command 量级与整个 `assistant-providers` crate 相当，目录拆分合理。在 `cmd/mod.rs` 中 `pub mod vault;` + `pub use vault::*;` 的写法与现有 `pub use chat::*;` 等完全一致，**外部感知不变**。

### 9.3 Rust `pub use` re-export 风格

**项目约定**（观察 `cmd/mod.rs`）：

```rust
pub use vault::*;
```

当前 `vault.rs` 中所有 `#[command]` 函数都是 `pub fn`，通过 `pub use vault::*` 被提升到 `cmd` 命名空间。

**拆分后**：`cmd/vault/mod.rs` 需要添加：

```rust
mod lifecycle;
mod entries;
mod reveal;
mod audit;
mod biometric;
mod sync;
mod ssh;
mod recovery;

pub use lifecycle::*;
pub use entries::*;
pub use reveal::*;
pub use audit::*;
pub use biometric::*;
pub use sync::*;
pub use ssh::*;
pub use recovery::*;
```

这与 `cmd/mod.rs` 中 `pub use vault::*;` 的模式 **完全一致**。✅ 已对齐。

### 9.4 Rust `invoke_handler` 注册风格

**项目约定**（观察 `src-tauri/src/lib.rs`）：

```rust
invoke_handler(tauri::generate_handler![
    cmd::vault_init,
    cmd::vault_unlock,
    // ... 55 个逐个列出
])
```

当前是 **逐个列出**，不是通配。

**拆分后**：由于 `pub use vault::*` re-export，`cmd::vault_init` 等路径 **完全不变**，`invoke_handler` 无需任何修改。✅ 已对齐。

### 9.5 前端 Store 文件组织风格

**项目约定**（观察 `src/stores/`）：

| store | 文件组织 |
|-------|---------|
| `chatStore` | `stores/chatStore.ts`（组装）+ `stores/chatCompactSlice.ts` + `stores/chatConversationSlice.ts` + `stores/chatMessageSlice.ts` + `stores/chatStorePorts.ts` |
| `settingsStore` | `stores/settingsStore.ts`（单文件，~100 行，无需拆分） |
| `historyStore` | `stores/historyStore.ts`（单文件，~100 行） |
| `orbStore` | `stores/orbStore.ts`（单文件，~50 行） |
| `providerStore` | `stores/providerStore.ts`（单文件，~80 行） |

**关键观察**：chatStore 的 slice 文件 **与 chatStore.ts 平铺在同一目录**（`stores/`），没有建 `slices/` 子目录。

**⚠️ 偏差 3**：文档中 vaultStore 拆分方案将 slice 放在 `vault/core/slices/` 子目录，而 chatStore 先例是 **平铺在同级目录**。

**修正方案**：两种选择——

| 方案 | 优点 | 缺点 |
|------|------|------|
| A. `vault/core/slices/` 子目录 | 7 个 slice 构成强内聚域，与 `settings/core/field/`、`settings/core/provider/` 子目录先例一致；`core/` 根层仅 5 个文件，干净 | 与 chatStore 先例不一致 |
| B. 平铺在 `vault/core/`（对齐 chatStore） | 与 chatStore 先例一致 | `vault/core/` 将有 13 个文件，slice 占 54%，视觉噪声大 |

**决策**：采用 **方案 A（`slices/` 子目录）**。理由：

1. **`vault/core/` 和 `stores/` 的定位不同**：`stores/` 是全局共享的"各模块 store 集散地"，不同 store 之间无内聚关系，平铺合理；而 `vault/core/` 是 **单一模块的内核**，其内部文件有强内聚关系，更接近 `settings/core/` 的场景
2. **7 个 slice 构成明确的内聚域**：它们共享 `FullState` 类型、共享 `set/get` 签名、互相引用字段——这和 `settings/core/field/` 内的 `fieldDefinitions.ts` + `fieldRegistry.ts` 是同构关系
3. **chatStore 只有 3 个 slice**，平铺的拥挤感不明显；vault 有 **7 个 slice**，平铺后 slice 占 `core/` 文件的 54%（7/13），视觉噪声显著
4. **import 路径多一层是微小代价**：`./slices/vaultLifecycleSlice` vs `./vaultLifecycleSlice`，差异可忽略

**更新后的目标结构**：

```
vault/core/
├── vaultStore.ts              ← slice 组装 + re-export（~80 行）
├── vault.types.ts             ← TS 类型定义（~80 行）
├── vault.service.ts           ← RPC 封装（~233 行）
├── useVaultEvents.ts          ← 不变（33 行）
├── useVaultAutoLock.ts        ← 不变（25 行）
├── slices/
│   ├── vaultLifecycleSlice.ts ← 生命周期 slice（~120 行）
│   ├── vaultEntriesSlice.ts   ← 条目 CRUD slice（~150 行）
│   ├── vaultRevealSlice.ts    ← 字段揭示 slice（~100 行）
│   ├── vaultAuditSlice.ts     ← TOTP + HIBP slice（~150 行）
│   ├── vaultBiometricSlice.ts ← 生物识别 slice（~100 行）
│   ├── vaultSyncSlice.ts      ← 同步 + SSH slice（~200 行）
│   └── vaultRecoverySlice.ts  ← 恢复 + 完整性 slice（~150 行）
└── vaultStore.test.ts         ← 测试
```

**命名约定**：chatStore slice 命名为 `chatCompactSlice.ts`、`chatConversationSlice.ts`、`chatMessageSlice.ts`（`chat` 前缀 + 域名 + `Slice` 后缀）。vault slice 应对齐为 `vaultLifecycleSlice.ts`、`vaultEntriesSlice.ts` 等。

### 9.6 前端 Slice 工厂函数签名风格

**项目约定**（观察 `chatCompactSlice.ts`、`chatConversationSlice.ts`、`chatMessageSlice.ts`）：

```ts
export function createCompactSlice(
  set: (partial: Partial<FullState> | ((state: FullState) => Partial<FullState>)) => void,
  get: () => FullState,
): ChatCompactSlice { ... }
```

关键特征：
1. 工厂函数接收 `set` + `get` 参数（不是 `(...a)` 解构）
2. `FullState` 在 slice 文件内用 `type FullState = XxxSlice & { ...跨 slice 字段 }` 定义
3. 返回类型是 slice 接口（如 `ChatCompactSlice`）
4. **不使用 Zustand middleware 参数**（无 `api` / `store`）

**⚠️ 偏差 4**：文档中 slice 组装写法为：

```ts
export const useVaultStore = create<VaultState>()((...a) => ({
  ...createLifecycleSlice()(...a),
  ...
}));
```

而 chatStore 实际写法为：

```ts
export const useChatStore = create<ChatState>()((set, get): ChatState => {
  const conversationSlice = createConversationSlice(set, get, ...);
  const messageSlice = createMessageSlice(set, get, ...);
  return { ...conversationSlice, ...messageSlice, ... };
});
```

**修正**：vault slice 组装应 **对齐 chatStore 模式**——在 `create` 回调内解构 `set, get`，传给各 slice 工厂。如果某些 slice 需要额外依赖（如 `PersistQueue`），也通过参数注入。

```ts
export const useVaultStore = create<VaultState>()((set, get): VaultState => {
  const lifecycleSlice = createLifecycleSlice(set, get);
  const entriesSlice = createEntriesSlice(set, get);
  const revealSlice = createRevealSlice(set, get);
  const auditSlice = createAuditSlice(set, get);
  const biometricSlice = createBiometricSlice(set, get);
  const syncSlice = createSyncSlice(set, get);
  const recoverySlice = createRecoverySlice(set, get);
  return {
    ...lifecycleSlice,
    ...entriesSlice,
    ...revealSlice,
    ...auditSlice,
    ...biometricSlice,
    ...syncSlice,
    ...recoverySlice,
  };
});
```

### 9.7 前端 Slice 接口命名风格

**项目约定**：chatStore slice 接口命名为 `ChatCompactSlice`、`ChatConversationSlice`、`ChatMessageSlice`（`Chat` 前缀 + 域名 + `Slice` 后缀）。

**⚠️ 偏差 5**：文档中 slice 接口命名为 `LifecycleState`、`EntriesState` 等（域名 + `State` 后缀，无 `Vault` 前缀）。

**修正**：对齐 chatStore 命名，改为 `VaultLifecycleSlice`、`VaultEntriesSlice`、`VaultRevealSlice`、`VaultAuditSlice`、`VaultBiometricSlice`、`VaultSyncSlice`、`VaultRecoverySlice`。

### 9.8 前端类型文件位置风格

**项目约定**（观察 chat 模块）：

| 模块 | 类型文件 | 位置 |
|------|---------|------|
| chat | `chatTypes.ts` | `src/chat/core/chatTypes.ts`（与 `chatErrors.ts`、`chatUtils.ts` 平铺） |
| settings | `confTypes.ts`、`fieldSchema.ts`、`pageTypes.ts` | `src/settings/core/types/`（独立 types 子目录） |
| mascot | `types.ts` | `src/mascot/core/types.ts` |
| provider | `types.ts` | `src/settings/provider/types.ts` |

**结论**：项目对类型文件的位置没有统一约定——有的平铺在 `core/`，有的放在 `types/` 子目录，有的就叫 `types.ts`。vault 当前把类型内联在 `vault.service.ts` 中。

**决策**：提取为 `vault/core/vault.types.ts`（平铺在 `core/`，与 `chatTypes.ts` 先例一致）。文件名用 `vault.types.ts`（带点号）而非 `vaultTypes.ts`（驼峰），与 `vault.service.ts`、`vaultStore.ts` 保持命名族一致。

### 9.9 前端 UI 组件目录风格

**项目约定**（观察各模块的组件组织）：

| 模块 | 组件目录 | 子目录模式 |
|------|---------|-----------|
| chat | `src/chat/` | `blocks/`、`compact/`、`core/`、`message/`、`selectors/` |
| history | `src/history/` | `core/`、`detail/`、`sidebar/` |
| mascot | `src/mascot/` | `core/`、`renderers/gif/`、`renderers/rive/`、`renderers/svg/` |
| settings | `src/settings/` | `core/`、`overview/`、`pages/`、`provider/`、`shared/` |
| vault | `src/vault/` | `core/`、`detail/`、`settings/` |

**关键观察**：
1. 项目广泛使用子目录组织组件（`blocks/`、`message/`、`renderers/` 等）
2. 子目录内组件 **不以父组件名做前缀**（如 `chat/blocks/` 内是 `EditBlock.tsx` 而非 `ChatEditBlock.tsx`）
3. vault 已有 `detail/` 和 `settings/` 子目录

**⚠️ 偏差 6**：文档中 VaultUnlock 拆分方案创建 `vault/unlock/` 子目录，VaultEntryDrawer 创建 `vault/entry-forms/` 子目录。命名风格与项目一致。但需注意：

- `unlock/` 内的组件命名应为 `PasswordForm.tsx`、`BiometricButton.tsx`、`RecoveryKeyForm.tsx`（无 `Vault` 前缀，与 `blocks/EditBlock.tsx` 先例一致），而非文档中的 `PasswordUnlockForm.tsx`、`BiometricUnlockButton.tsx`（冗余的 `Unlock` 中缀）
- `entry-forms/` 应改为 `forms/`（更简洁，与 `blocks/`、`renderers/` 先例一致）

**修正后的 UI 拆分结构**：

```
vault/
├── VaultApp.tsx                 ← 不变
├── VaultUnlock.tsx              ← 容器（~80 行）
├── VaultList.tsx                ← 不变
├── VaultEntryDrawer.tsx         ← 容器（~120 行）
├── unlock/
│   ├── PasswordForm.tsx         ← 主密码输入 + 提交（~100 行）
│   ├── BiometricButton.tsx      ← 生物识别按钮（~80 行）
│   └── RecoveryKeyForm.tsx      ← 恢复密钥输入 + 提交（~100 行）
├── forms/
│   ├── LoginFields.tsx          ← username + url + password（~60 行）
│   ├── NoteFields.tsx           ← notes（~40 行）
│   ├── CardFields.tsx           ← cardholder + number + expiry + cvv（~80 行）
│   ├── IdentityFields.tsx       ← identity 字段（~60 行）
│   ├── SshKeyFields.tsx         ← private key + public key（~60 行）
│   └── CustomFields.tsx         ← custom fields 动态表单（~80 行）
├── detail/
│   ├── VaultEntryDetail.tsx     ← 容器（~120 行）
│   ├── TotpDisplay.tsx          ← TOTP 倒计时（~80 行）
│   ├── HibpBadge.tsx            ← HIBP 泄露标记（~30 行）
│   └── RevealFieldButton.tsx    ← 揭示按钮（~50 行）
├── settings/
│   ├── VaultSettings.tsx        ← Tabs 容器（~60 行）
│   ├── VaultSyncSettings.tsx    ← 不变
│   ├── VaultConflictResolve.tsx ← 不变
│   ├── SshAgentTab.tsx          ← SSH Agent tab（~80 行）
│   ├── RecoveryTab.tsx          ← 恢复密钥 + 完整性 tab（~80 行）
│   ├── ImportExportTab.tsx      ← 导入/导出 tab（~80 行）
│   └── DangerZoneConfirm.tsx    ← 不变
└── core/
    ├── vaultStore.ts            ← slice 组装
    ├── vault.types.ts           ← 类型定义
    ├── vault.service.ts         ← RPC 封装
    ├── useVaultEvents.ts        ← 不变
    ├── useVaultAutoLock.ts      ← 不变
    ├── slices/
    │   ├── vaultLifecycleSlice.ts   ← 生命周期 slice
    │   ├── vaultEntriesSlice.ts     ← 条目 CRUD slice
    │   ├── vaultRevealSlice.ts      ← 字段揭示 slice
    │   ├── vaultAuditSlice.ts       ← TOTP + HIBP slice
    │   ├── vaultBiometricSlice.ts   ← 生物识别 slice
    │   ├── vaultSyncSlice.ts        ← 同步 + SSH slice
    │   └── vaultRecoverySlice.ts    ← 恢复 + 完整性 slice
    └── vaultStore.test.ts       ← 测试
```

### 9.10 前端 CSS Module 风格

**项目约定**：vault 使用单一 CSS Module 文件 `vault/vault.module.css`，所有子组件共享同一 `styles` 对象。

**拆分后**：新增的子组件（`unlock/`、`forms/`、`detail/` 内的新文件）继续从 `../vault.module.css` 或 `../../vault.module.css` 导入 `styles`，**不创建新的 CSS Module 文件**。这与 chat 模块的模式一致（`ChatView.tsx` 和 `blocks/EditBlock.tsx` 都从各自的 CSS Module 导入，但 vault 已有统一 CSS 文件且体量不大）。

### 9.11 Rust 测试文件位置风格

**项目约定**（观察 `assistant-security/tests/`）：

```
tests/
├── hibp.rs
├── password_strength.rs
├── recovery_key.rs
├── reveal_guard.rs
├── security_db.rs
├── totp.rs
├── vault_crud.rs
├── vault_crypto.rs
```

测试文件在 `tests/` 目录（集成测试），不在 `src/` 内。`security_db.rs` 测试通过 `use assistant_security::security_db::SecurityDbState` 引用。

**拆分后**：测试文件 **无需改动**，因为 `SecurityDbState` 的 `pub` 方法路径不变（`assistant_security::security_db::SecurityDbState` 仍有效，子模块方法通过 `mod.rs` 的 `impl` 块可见）。✅ 已对齐。

### 9.12 前端测试文件风格

**项目约定**（观察 `vaultStore.test.ts`）：

- 测试文件与源文件 **同目录**（`vault/core/vaultStore.test.ts`）
- 使用 `vi.mock` mock `tauriInvoke`
- `resetStore()` 直接 `useVaultStore.setState()` 重置全量 state

**拆分后**：`vaultStore.test.ts` 仍放在 `vault/core/`，但需要更新：
1. `resetStore()` 中的 `setState` 调用不变（全量 state 类型 `VaultState` 仍由 `vaultStore.ts` 导出）
2. 可新增按 slice 的测试文件（如 `vaultLifecycleSlice.test.ts`），但非必须

### 9.13 Rust `with_conn_and_key` 可见性

**当前**：`with_conn_and_key` 是 `fn`（crate-private），不是 `pub fn`。

**拆分后**：子模块文件（`crud.rs`、`crypto_ops.rs` 等）需要调用 `self.with_conn_and_key()`。由于它们在同一个 crate 内且通过 `impl SecurityDbState` 调用，`fn` 可见性足够（Rust 允许同一 crate 内的 `impl` 块访问 crate-private 方法）。

但需注意：`with_conn_and_key` 定义在 `mod.rs` 的 `impl SecurityDbState` 块中，子模块的 `impl SecurityDbState` 块可以调用它，因为 Rust 的可见性规则是 **按 crate 而非按文件**。✅ 已对齐。

### 9.14 Rust `unlock_with_master_key` 引用

**当前**：`security_db/mod.rs` 第 1360 行调用了 `self.unlock_with_master_key(master_key)?;`，但该函数 **在整个 crate 中没有定义**（grep 未找到）。这可能是编译通过但未被触发的死代码路径（仅在 `#[cfg(target_os = "macos")]` 分支），或是一个待实现的 TODO。

**拆分后**：此调用位于 `biometric_ops.rs` 的 `unlock_biometric` 方法中。拆分时应 **保留原样**，不改变此逻辑。但建议在拆分 PR 中标注此问题，后续单独修复。

### 9.15 `custom_fields_eq` 辅助函数归属

**当前**：`custom_fields_eq` 定义在 `security_db/mod.rs` 末尾（第 1565-1579 行），是模块级自由函数（非 `impl` 方法）。

**调用位置**：仅在 `update_entry` 和 `import_entries` 中使用。

**文档方案**：移入 `import_export.rs`。

**修正**：`update_entry` 在 `crud.rs` 中也使用了 `custom_fields_eq`。因此该函数应 **留在 `mod.rs` 作为 `pub(crate) fn`**，或移入 `crud.rs` 并 `pub(crate)` 导出。鉴于它仅 15 行且被两个子模块使用，留在 `mod.rs` 更合理。

### 9.16 汇总：需修正的偏差

| # | 偏差 | 修正 | 影响章节 |
|---|------|------|---------|
| 2 | `cmd/vault/` 是 cmd 层首个目录 | 接受，vault 55 个 command 量级合理 | §3.2 |
| 3 | slice 放 `slices/` 子目录 vs chatStore 平铺 | **保留 `slices/` 子目录**：7 个 slice 构成强内聚域，与 `settings/core/` 子目录先例一致；chatStore 仅 3 个 slice 平铺可行，vault 7 个平铺噪声过大 | §4.1 |
| 4 | slice 组装用 `(...a)` 解构 vs chatStore 用 `(set, get)` | **改为 `(set, get)` 显式参数** | §4.1 |
| 5 | slice 接口命名 `LifecycleState` vs chatStore `ChatCompactSlice` | **改为 `VaultLifecycleSlice` 等** | §4.1 |
| 6 | UI 子组件冗余前缀 + `entry-forms/` 命名 | **改为 `unlock/PasswordForm.tsx`、`forms/LoginFields.tsx` 等** | §4.3 |
| — | `custom_fields_eq` 归属 `import_export.rs` | **改为留在 `mod.rs` 作 `pub(crate) fn`** | §3.1 |
| — | `vault.types.ts` 命名 | 确认用点号分隔（`vault.types.ts`），与 `vault.service.ts` 一致 | §4.2 |
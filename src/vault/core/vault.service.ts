import { tauriInvoke as invoke } from "@/services/tauriInvoke";
import type {
  VaultEntrySummary,
  VaultEntryDetail,
  RevealResult,
  VaultEntryCreate,
  VaultEntryUpdate,
  TotpResult,
  ParsedOtpAuth,
  AuditLogEntry,
  StrengthResult,
  SyncResult,
  SyncConfig,
  Readyness,
  ConflictResolution,
  SshKeyInfo,
  IntegrityResult,
  DisasterRecoveryResult,
} from "./vault.types";

export type {
  VaultCategory,
  VaultEntrySummary,
  VaultEntryDetail,
  RevealResult,
  CustomFieldInput,
  VaultEntryCreate,
  VaultEntryUpdate,
  TotpResult,
  ParsedOtpAuth,
  AuditLogEntry,
  StrengthResult,
  Readyness,
  RepositoryState,
  SyncResult,
  ConflictFile,
  SyncConfig,
  ConflictResolution,
  SshKeyInfo,
  IntegrityResult,
  DisasterRecoveryResult,
  RevealedField,
} from "./vault.types";

export const vaultService = {
  init: (masterPassword: string): Promise<void> =>
    invoke("vault_init", { masterPassword }),

  unlock: (masterPassword: string): Promise<boolean> =>
    invoke("vault_unlock", { masterPassword }),

  isLocked: (): Promise<boolean> => invoke("vault_is_locked"),

  isInitialized: (): Promise<boolean> => invoke("vault_is_initialized"),

  lock: (): Promise<void> => invoke("vault_lock"),

  listEntries: (
    category?: string,
    search?: string,
    tag?: string,
  ): Promise<VaultEntrySummary[]> =>
    invoke("vault_list_entries", { category, search, tag }),

  getEntry: (id: string): Promise<VaultEntryDetail> =>
    invoke("vault_get_entry", { id }),

  copyPassword: (id: string): Promise<void> =>
    invoke("vault_copy_password", { id }),

  copyField: (id: string, fieldName: string): Promise<void> =>
    invoke("vault_copy_field", { id, fieldName }),

  revealPassword: (id: string): Promise<RevealResult> =>
    invoke("vault_reveal_password", { id }),

  revealNotes: (id: string): Promise<RevealResult> =>
    invoke("vault_reveal_notes", { id }),

  createEntry: (entry: VaultEntryCreate): Promise<string> =>
    invoke("vault_create_entry", { entry }),

  updateEntry: (id: string, entry: VaultEntryUpdate): Promise<void> =>
    invoke("vault_update_entry", { id, entry }),

  deleteEntry: (id: string): Promise<void> =>
    invoke("vault_delete_entry", { id }),

  changeMasterPassword: (current: string, newPassword: string): Promise<void> =>
    invoke("vault_change_master_password", { current, new: newPassword }),

  exportVault: (format: "csv" | "json"): Promise<string> =>
    invoke("vault_export", { format }),

  exportVaultToFile: (format: "csv" | "json", path: string): Promise<void> =>
    invoke("vault_export_to_file", { format, path }),

  importVault: (path: string, format: "csv" | "json"): Promise<number> =>
    invoke("vault_import", { path, format }),

  totpGenerate: (
    id: string,
    period?: number,
    digits?: number,
    algorithm?: string,
  ): Promise<TotpResult> =>
    invoke("vault_totp_generate", { id, period, digits, algorithm }),

  totpCopy: (
    id: string,
    period?: number,
    digits?: number,
    algorithm?: string,
  ): Promise<void> =>
    invoke("vault_copy_totp", { id, period, digits, algorithm }),

  totpTimeRemaining: (period?: number): Promise<number> =>
    invoke("vault_totp_time_remaining", { period }),

  totpParseUri: (uri: string): Promise<ParsedOtpAuth> =>
    invoke("vault_totp_parse_uri", { uri }),

  queryAuditLog: (limit?: number): Promise<AuditLogEntry[]> =>
    invoke("vault_query_audit_log", { limit }),

  showUnlockWindow: (): Promise<void> => invoke("vault_show_unlock_window"),

  hibpCheck: (id: string): Promise<number> =>
    invoke("vault_hibp_check", { id }),

  hibpAudit: (): Promise<[string, number][]> => invoke("vault_hibp_audit"),

  checkMasterPasswordStrength: (password: string): Promise<StrengthResult> =>
    invoke("vault_check_master_password_strength", { password }),

  isBiometricAvailable: (): Promise<boolean> =>
    invoke("vault_is_biometric_available"),

  isBiometricEnrolled: (): Promise<boolean> =>
    invoke("vault_is_biometric_enrolled"),

  unlockBiometric: (): Promise<boolean> => invoke("vault_unlock_biometric"),

  enrollBiometric: (masterPassword: string): Promise<void> =>
    invoke("vault_enroll_biometric", { masterPassword }),

  disenrollBiometric: (): Promise<void> => invoke("vault_disenroll_biometric"),

  syncPull: (): Promise<SyncResult> => invoke("vault_sync_pull"),
  syncPush: (): Promise<SyncResult> => invoke("vault_sync_push"),
  syncNow: (): Promise<SyncResult> => invoke("vault_sync_now"),
  syncForcePush: (): Promise<SyncResult> => invoke("vault_sync_force_push"),
  getSyncConfig: (): Promise<SyncConfig> => invoke("vault_get_sync_config"),
  setSyncConfig: (config: SyncConfig): Promise<void> =>
    invoke("vault_set_sync_config", { config }),
  resolveConflict: (
    path: string,
    resolution: ConflictResolution,
  ): Promise<void> => invoke("vault_resolve_conflict", { path, resolution }),
  gitInit: (): Promise<void> => invoke("vault_git_init"),
  gitClone: (url: string): Promise<void> => invoke("vault_git_clone", { url }),
  gitAddRemote: (remote: string, url: string): Promise<void> =>
    invoke("vault_git_add_remote", { remote, url }),
  gitSetRemote: (remote: string, url: string): Promise<void> =>
    invoke("vault_git_set_remote", { remote, url }),
  syncReadyness: (): Promise<Readyness> => invoke("vault_sync_readyness"),
  syncPrepare: (): Promise<void> => invoke("vault_sync_prepare"),
  syncFinalize: (message: string): Promise<void> =>
    invoke("vault_sync_finalize", { message }),

  sshAddToAgent: (id: string, lifetime?: number): Promise<string> =>
    invoke("vault_ssh_add_to_agent", { id, lifetime }),

  sshRemoveFromAgent: (fingerprint: string): Promise<void> =>
    invoke("vault_ssh_remove_from_agent", { fingerprint }),

  sshListAgent: (): Promise<SshKeyInfo[]> => invoke("vault_ssh_list_agent"),

  sshAutoAddAll: (): Promise<string[]> => invoke("vault_ssh_auto_add_all"),

  sshIsAgentRunning: (): Promise<boolean> =>
    invoke("vault_ssh_is_agent_running"),

  generateRecoveryKey: (): Promise<string> =>
    invoke("vault_generate_recovery_key"),

  unlockWithRecoveryKey: (recoveryKey: string): Promise<boolean> =>
    invoke("vault_unlock_with_recovery_key", { recoveryKey }),

  hasRecoveryKey: (): Promise<boolean> => invoke("vault_has_recovery_key"),

  verifyVaultMetaIntegrity: (): Promise<IntegrityResult> =>
    invoke("vault_verify_vault_meta_integrity"),

  disasterRecoveryExport: (
    gitUrl: string | null,
    recoveryKey: string,
    exportPath: string,
  ): Promise<DisasterRecoveryResult> =>
    invoke("vault_disaster_recovery_export", {
      gitUrl,
      recoveryKey,
      exportPath,
    }),
};

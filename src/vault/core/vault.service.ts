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
  ConflictFile,
  PendingRemovals,
  HistoryCommit,
  EntryPreview,
  ConflictPreview,
  SshKeyInfo,
  SshKeyMeta,
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
  HistoryCommit,
  EntryPreview,
  ConflictPreview,
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

  listDeletedEntries: (): Promise<VaultEntrySummary[]> =>
    invoke("vault_list_deleted_entries"),

  restoreEntry: (id: string): Promise<void> =>
    invoke("vault_restore_entry", { id }),

  purgeDeleted: (): Promise<number> => invoke("vault_purge_deleted"),

  changeMasterPassword: (current: string, newPassword: string): Promise<void> =>
    invoke("vault_change_master_password", { current, new: newPassword }),

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

  showUnlockWindow: (): Promise<boolean> => invoke("vault_show_unlock_window"),

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

  enrollBiometric: (): Promise<void> => invoke("vault_enroll_biometric"),

  disenrollBiometric: (): Promise<void> => invoke("vault_disenroll_biometric"),

  syncPull: (): Promise<SyncResult> => invoke("vault_sync", { action: "pull" }),
  syncPush: (): Promise<SyncResult> => invoke("vault_sync", { action: "push" }),
  syncNow: (): Promise<SyncResult> => invoke("vault_sync", { action: "sync" }),
  syncForcePush: (): Promise<SyncResult> =>
    invoke("vault_sync", { action: "forcePush" }),
  syncLastTime: (): Promise<number | null> => invoke("vault_sync_last_time"),
  getSyncConfig: (): Promise<SyncConfig> => invoke("vault_get_sync_config"),
  setSyncConfig: (config: SyncConfig): Promise<void> =>
    invoke("vault_set_sync_config", { config }),
  resolveConflict: (
    path: string,
    resolution: ConflictResolution,
    remoteCopySuffix?: string,
  ): Promise<void> =>
    invoke("vault_resolve_conflict", { path, resolution, remoteCopySuffix }),
  gitInit: (): Promise<void> => invoke("vault_git_init"),
  gitClone: (url: string): Promise<void> => invoke("vault_git_init", { url }),
  gitAddRemote: (remote: string, url: string): Promise<void> =>
    invoke("vault_git_remote", { remote, url, mode: "add" }),
  gitSetRemote: (remote: string, url: string): Promise<void> =>
    invoke("vault_git_remote", { remote, url, mode: "set" }),
  gitRemoveRemote: (remote: string): Promise<void> =>
    invoke("vault_git_remove_remote", { remote }),
  gitTestConnection: (url?: string): Promise<number> =>
    invoke("vault_git_test_connection", { url: url ?? null }),
  syncAbort: (): Promise<void> => invoke("vault_sync_abort"),
  syncDiscard: (): Promise<void> => invoke("vault_sync_discard"),
  syncReadyness: (): Promise<Readyness> => invoke("vault_sync_readyness"),
  syncConflicts: (): Promise<ConflictFile[]> => invoke("vault_sync_conflicts"),
  syncPendingRemovals: (): Promise<PendingRemovals | null> =>
    invoke("vault_sync_pending_removals"),
  syncConfirmRemovals: (ids: string[]): Promise<number> =>
    invoke("vault_sync_confirm_removals", { ids }),

  historyList: (entryId?: string, limit?: number): Promise<HistoryCommit[]> =>
    invoke("vault_history_list", {
      entryId: entryId ?? null,
      limit: limit ?? null,
    }),

  historyPreview: (commit: string, entryId: string): Promise<EntryPreview> =>
    invoke("vault_history_preview", { commit, entryId }),

  historyRestore: (commit: string, entryId: string): Promise<string> =>
    invoke("vault_history_restore", { commit, entryId }),

  conflictPreview: (path: string): Promise<ConflictPreview> =>
    invoke("vault_conflict_preview", { path }),

  revealConflictField: (
    path: string,
    side: "local" | "remote",
    field: string,
  ): Promise<string> => invoke("vault_conflict_reveal", { path, side, field }),

  sshAddToAgent: (id: string, lifetime?: number): Promise<string> =>
    invoke("vault_ssh_add_to_agent", { id, lifetime }),

  sshRemoveFromAgent: (fingerprint: string): Promise<void> =>
    invoke("vault_ssh_remove_from_agent", { fingerprint }),

  sshListAgent: (): Promise<SshKeyInfo[]> => invoke("vault_ssh_list_agent"),

  sshAutoAddAll: (): Promise<string[]> => invoke("vault_ssh_auto_add_all"),

  sshIsAgentRunning: (): Promise<boolean> =>
    invoke("vault_ssh_is_agent_running"),

  sshParseKey: (privateKey: string): Promise<SshKeyMeta> =>
    invoke("vault_ssh_parse_key", { privateKey }),

  sshKeyMeta: (id: string): Promise<SshKeyMeta> =>
    invoke("vault_ssh_key_meta", { id }),

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
    commit?: string | null,
  ): Promise<DisasterRecoveryResult> =>
    invoke("vault_disaster_recovery_export", {
      gitUrl,
      recoveryKey,
      exportPath,
      commit: commit ?? null,
    }),
};

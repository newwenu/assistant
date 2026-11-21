import { translateKey, type I18nPath } from "@/i18n";

export const VAULT_AUDIT_EVENT_TYPES = [
  "vault_unlock",
  "vault_unlock_failed",
  "vault_lock",
  "vault_reveal",
  "vault_reveal_rate_limited",
  "vault_entry_created",
  "vault_entry_updated",
  "vault_entry_deleted",
  "vault_entry_restored",
  "vault_entry_purged",
  "vault_copy_password",
  "vault_copy_field",
  "hibp_audit",
  "hibp_leak_found",
  "master_password_change",
  "recovery_key_exported",
  "recovery_export_used",
  "dek_rotated",
  "local_db_rebuilt",
  "anchor_rollback_rejected",
  "biometric_enrolled",
  "biometric_unlocked",
  "biometric_failed",
  "biometric_revoked",
  "ssh_key_loaded",
  "ssh_key_removed",
  "vault_repository_reset",
  "vault_unsupported_version",
  "vault_entries_exported",
  "vault_entries_imported",
] as const;

export type VaultAuditEventType = (typeof VAULT_AUDIT_EVENT_TYPES)[number];

const EVENT_TEXT_KEYS = {
  vault_unlock: "vault.security_log.event.vault_unlock",
  vault_unlock_failed: "vault.security_log.event.vault_unlock_failed",
  vault_lock: "vault.security_log.event.vault_lock",
  vault_reveal: "vault.security_log.event.vault_reveal",
  vault_reveal_rate_limited:
    "vault.security_log.event.vault_reveal_rate_limited",
  vault_entry_created: "vault.security_log.event.vault_entry_created",
  vault_entry_updated: "vault.security_log.event.vault_entry_updated",
  vault_entry_deleted: "vault.security_log.event.vault_entry_deleted",
  vault_entry_restored: "vault.security_log.event.vault_entry_restored",
  vault_entry_purged: "vault.security_log.event.vault_entry_purged",
  vault_copy_password: "vault.security_log.event.vault_copy_password",
  vault_copy_field: "vault.security_log.event.vault_copy_field",
  hibp_audit: "vault.security_log.event.hibp_audit",
  hibp_leak_found: "vault.security_log.event.hibp_leak_found",
  master_password_change: "vault.security_log.event.master_password_change",
  recovery_key_exported: "vault.security_log.event.recovery_key_exported",
  recovery_export_used: "vault.security_log.event.recovery_export_used",
  dek_rotated: "vault.security_log.event.dek_rotated",
  local_db_rebuilt: "vault.security_log.event.local_db_rebuilt",
  anchor_rollback_rejected: "vault.security_log.event.anchor_rollback_rejected",
  biometric_enrolled: "vault.security_log.event.biometric_enrolled",
  biometric_unlocked: "vault.security_log.event.biometric_unlocked",
  biometric_failed: "vault.security_log.event.biometric_failed",
  biometric_revoked: "vault.security_log.event.biometric_revoked",
  ssh_key_loaded: "vault.security_log.event.ssh_key_loaded",
  ssh_key_removed: "vault.security_log.event.ssh_key_removed",
  vault_repository_reset: "vault.security_log.event.vault_repository_reset",
  vault_unsupported_version:
    "vault.security_log.event.vault_unsupported_version",
  vault_entries_exported: "vault.security_log.event.vault_entries_exported",
  vault_entries_imported: "vault.security_log.event.vault_entries_imported",
} as const satisfies Record<VaultAuditEventType, I18nPath>;

function lookupKey(
  keys: Readonly<Record<string, I18nPath>>,
  value: string,
): I18nPath | undefined {
  return Object.prototype.hasOwnProperty.call(keys, value)
    ? keys[value]
    : undefined;
}

export function vaultAuditEventText(eventType: string): string {
  const key = lookupKey(EVENT_TEXT_KEYS, eventType);
  return key === undefined ? eventType : translateKey(key);
}

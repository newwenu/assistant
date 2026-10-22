import { tauriInvoke as invoke } from "@/services/tauriInvoke";

export type VaultCategory =
  "login" | "note" | "card" | "identity" | "sshKey" | "custom";

export interface VaultEntrySummary {
  id: string;
  category: VaultCategory;
  title: string;
  username: string | null;
  url: string | null;
  tags: string[];
  isFavorite: boolean;
  updatedAt: number;
}

export interface VaultEntryDetail {
  id: string;
  category: VaultCategory;
  title: string;
  username: string | null;
  url: string | null;
  hasPassword: boolean;
  hasNotes: boolean;
  hasTotp: boolean;
  customFieldNames: string[];
  tags: string[];
  isFavorite: boolean;
  revision: number;
  createdAt: number;
  updatedAt: number;
}

export interface RevealResult {
  value: string;
  expiresAt: number;
}

export interface CustomFieldInput {
  name: string;
  value: string;
  isProtected: boolean;
}

export interface VaultEntryCreate {
  category: VaultCategory;
  title: string;
  username?: string;
  url?: string;
  password?: string;
  notes?: string;
  totpSecret?: string;
  tags: string[];
  customFields: CustomFieldInput[];
  isFavorite: boolean;
}

export interface VaultEntryUpdate {
  category: VaultCategory;
  title: string;
  username?: string;
  url?: string;
  password?: string;
  notes?: string;
  totpSecret?: string;
  tags: string[];
  customFields: CustomFieldInput[];
  isFavorite: boolean;
}

export interface TotpResult {
  code: string;
  timeRemaining: number;
}

export interface StrengthResult {
  score: number;
  entropyBits: number;
  crackTimeSeconds: number;
  crackTimeDisplay: string;
  warnings: string[];
}

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

  importVault: (path: string, format: "csv" | "json"): Promise<number> =>
    invoke("vault_import", { path, format }),

  totpGenerate: (
    id: string,
    period?: number,
    digits?: number,
    algorithm?: string,
  ): Promise<TotpResult> =>
    invoke("vault_totp_generate", { id, period, digits, algorithm }),

  totpTimeRemaining: (period?: number): Promise<number> =>
    invoke("vault_totp_time_remaining", { period }),

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
};

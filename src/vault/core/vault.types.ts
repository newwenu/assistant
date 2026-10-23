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

export interface ParsedOtpAuth {
  secret: string;
  digits: number;
  period: number;
  algorithm: string;
  issuer: string | null;
  account: string | null;
}

export interface AuditLogEntry {
  id: string;
  eventType: string;
  detail: string | null;
  timestamp: number;
}

export interface StrengthResult {
  score: number;
  entropyBits: number;
  crackTimeSeconds: number;
  crackTimeDisplay: string;
  warnings: string[];
}

export type Readyness =
  "noSync" | { repoState: RepositoryState } | "dirty" | "ready";

export type RepositoryState =
  | "clean"
  | "merge"
  | "rebase"
  | "rebaseInteractive"
  | "rebaseMerge"
  | "apply"
  | "cherryPick"
  | "revert"
  | "bisect"
  | "am"
  | "mailbox"
  | "stash"
  | "unknown";

export type SyncResult =
  "success" | "upToDate" | { conflict: ConflictFile[] } | { error: string };

export interface ConflictFile {
  path: string;
  localHash: string;
  remoteHash: string;
}

export interface SyncConfig {
  remoteUrl: string | null;
  remoteName: string;
  branch: string;
  sshKeyPath: string | null;
}

export type ConflictResolution =
  "useLocal" | "useRemote" | "mergeBoth" | "skip";

export interface SshKeyInfo {
  fingerprint: string;
  keyType: string;
  comment: string;
  bits: number;
  addedByVault: boolean;
}

export interface IntegrityResult {
  valid: boolean;
  message: string;
}

export interface DisasterRecoveryResult {
  filePath: string;
  entryCount: number;
  integrityWarning: boolean;
}

export interface RevealedField {
  value: string;
  expiresAt: number;
  kind: "password" | "notes";
}

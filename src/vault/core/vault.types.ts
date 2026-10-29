/**
 * 时间戳单位约定：本文件所有 `timestamp` / `*At` 字段均为 **unix 毫秒**，
 * 唯一例外是 `.vault-meta` 文件内的 `exportedAt`（unix 秒，见 vault_file/types.rs）。
 */
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
  /** unix 毫秒 */
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

export interface PendingRemovalEntry {
  id: string;
  title: string | null;
}

export interface PendingRemovals {
  entries: PendingRemovalEntry[];
  totalLocal: number;
}

export type UnreadableReason = "keyMismatch" | "corrupt";

export interface UnreadableEntries {
  ids: string[];
  reason: UnreadableReason;
}

export type SyncResult =
  | "success"
  | "upToDate"
  | { conflict: ConflictFile[] }
  | { pendingRemovals: PendingRemovals }
  | { unreadableEntries: UnreadableEntries }
  | { error: string };

export interface ConflictFile {
  path: string;
  localHash: string;
  remoteHash: string;
  title: string | null;
}

export interface SyncConfig {
  remoteUrl: string | null;
  remoteName: string;
  sshKeyPath: string | null;
}

export type ConflictResolution =
  "useLocal" | "useRemote" | "mergeBoth" | "skip";

export interface HistoryCommit {
  hash: string;
  /** unix 毫秒 */
  timestamp: number;
  message: string;
  entryId: string | null;
}

export interface EntryPreviewField {
  name: string;
  value: string;
  isSecret: boolean;
}

export interface EntryPreview {
  id: string;
  commit: string;
  commitAuthor: string | null;
  /** unix 毫秒（由 Git `%ct` 秒值 ×1000，低三位恒为 000） */
  commitTime: number | null;
  category: VaultCategory;
  revision: number;
  title: string;
  username: string | null;
  url: string | null;
  password: string | null;
  notes: string | null;
  totpSecret: string | null;
  customFields: EntryPreviewField[];
  tags: string[];
  isFavorite: boolean;
  /** unix 毫秒 */
  createdAt: number;
  /** unix 毫秒 */
  updatedAt: number;
}

export interface ConflictPreview {
  entryId: string;
  local: EntryPreview | null;
  remote: EntryPreview | null;
  localError: string | null;
  remoteError: string | null;
  diffFields: string[];
}

export interface SshKeyInfo {
  fingerprint: string;
  keyType: string;
  comment: string;
  bits: number;
  addedByVault: boolean;
}

export interface SshKeyMeta {
  keyType: string;
  bits: number;
  fingerprint: string;
  comment: string;
  publicKey: string;
}

export interface IntegrityResult {
  valid: boolean;
  message: string;
}

export interface DisasterRecoveryResult {
  filePath: string;
  entryCount: number;
  integrityWarning: boolean;
  skippedIds: string[];
}

export interface RevealedField {
  value: string;
  expiresAt: number;
  kind: "password" | "notes";
}

/**
 * 时间戳单位约定：本文件所有 `timestamp` / `*At` 字段均为 **unix 毫秒**，
 * 唯一例外是 `.vault-meta` 文件内的 `exportedAt`（unix 秒，见 vault_file/types.rs）。
 *
 * 本文件为 barrel：除 `RevealedField`（纯前端类型）外，全部类型来自
 * `./contract/vault.contract`（由 ts-rs 从 Rust dto 生成，勿手改）。
 */
export * from "./contract/vault.contract";

export type { ConflictFileDto as ConflictFile } from "./contract/vault.contract";
export type { ConflictPreviewDto as ConflictPreview } from "./contract/vault.contract";
export type { ConflictResolutionDto as ConflictResolution } from "./contract/vault.contract";
export type { EntryPreviewDto as EntryPreview } from "./contract/vault.contract";
export type { EntryPreviewFieldDto as EntryPreviewField } from "./contract/vault.contract";
export type { ExportResultDto as ExportResult } from "./contract/vault.contract";
export type { HibpAuditEntryDto as HibpAuditEntry } from "./contract/vault.contract";
export type { HistoryCommitDto as HistoryCommit } from "./contract/vault.contract";
export type { ParsedOtpAuthDto as ParsedOtpAuth } from "./contract/vault.contract";
export type { PendingRemovalEntryDto as PendingRemovalEntry } from "./contract/vault.contract";
export type { PendingRemovalsDto as PendingRemovals } from "./contract/vault.contract";
export type { ReadynessDto as Readyness } from "./contract/vault.contract";
export type { RemoteModeDto as RemoteMode } from "./contract/vault.contract";
export type { RemoteStatusDto as RemoteStatus } from "./contract/vault.contract";
export type { RemoteStatusErrorKindDto as RemoteStatusErrorKind } from "./contract/vault.contract";
export type { RepositoryStateDto as RepositoryState } from "./contract/vault.contract";
export type { SshKeyInfoDto as SshKeyInfo } from "./contract/vault.contract";
export type { SshKeyMetaDto as SshKeyMeta } from "./contract/vault.contract";
export type { SyncActionDto as SyncAction } from "./contract/vault.contract";
export type { SyncConfigDto as SyncConfig } from "./contract/vault.contract";
export type { SyncResultDto as SyncResult } from "./contract/vault.contract";
export type { TotpAlgorithmDto as TotpAlgorithm } from "./contract/vault.contract";
export type { TotpCodeDto as TotpCode } from "./contract/vault.contract";
export type { TotpInputDto as TotpInput } from "./contract/vault.contract";
export type { UnreadableEntriesDto as UnreadableEntries } from "./contract/vault.contract";
export type { UnreadableReasonDto as UnreadableReason } from "./contract/vault.contract";

export interface RevealedField {
  entryId: string;
  value: string;
  expiresAt: number;
  kind: "password" | "notes";
  visible: boolean;
}

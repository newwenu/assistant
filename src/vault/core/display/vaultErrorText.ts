import { translateKey, type I18nPath } from "@/i18n";
import type { VaultErrorCode } from "../vault.types";

const ERROR_TEXT_KEYS = {
  locked: "vault.error.locked",
  invalidPassword: "vault.error.invalidPassword",
  notInitialized: "vault.error.notInitialized",
  alreadyExists: "vault.error.alreadyExists",
  rateLimited: "vault.error.rateLimited",
  crypto: "vault.error.crypto",
  notFound: "vault.error.notFound",
  entryCorrupted: "vault.error.entryCorrupted",
  validation: "vault.error.validation",
  biometricUnavailable: "vault.error.biometricUnavailable",
  biometricCancelled: "vault.error.biometricCancelled",
  biometricFailed: "vault.error.biometricFailed",
  biometricReset: "vault.error.biometricReset",
  invalidKeyFormat: "vault.error.invalidKeyFormat",
  invalidRecoveryKeyFormat: "vault.error.invalidRecoveryKeyFormat",
  invalidRecoveryKeyChecksum: "vault.error.invalidRecoveryKeyChecksum",
  invalidRecoveryKeyOrMetaTampered:
    "vault.error.invalidRecoveryKeyOrMetaTampered",
  vaultMetaIntegrityFailed: "vault.error.vaultMetaIntegrityFailed",
  unsupportedVaultVersion: "vault.error.unsupportedVaultVersion",
  vaultVersionTooNew: "vault.error.vaultVersionTooNew",
  localDbKeyMismatch: "vault.error.localDbKeyMismatch",
  vaultRollbackDetected: "vault.error.vaultRollbackDetected",
  vaultManifestMismatch: "vault.error.vaultManifestMismatch",
  vaultCorrupted: "vault.error.vaultCorrupted",
  cancelled: "vault.error.cancelled",
  repoNotInitialized: "vault.error.repoNotInitialized",
  noRemoteConfigured: "vault.error.noRemoteConfigured",
  noUpstream: "vault.error.noUpstream",
  syncConflict: "vault.error.syncConflict",
  gitUnavailable: "vault.error.gitUnavailable",
  keyMismatch: "vault.error.keyMismatch",
  historyDecryptFailed: "vault.error.historyDecryptFailed",
  vaultDekVersionAhead: "vault.error.vaultDekVersionAhead",
  vaultDekVersionBehind: "vault.error.vaultDekVersionBehind",
  repositoryContainsForbiddenFile:
    "vault.error.repositoryContainsForbiddenFile",
  historyDekUnavailable: "vault.error.historyDekUnavailable",
  database: "vault.error.database",
  io: "vault.error.io",
  clipboard: "vault.error.clipboard",
  alreadyUnlocked: "vault.error.alreadyUnlocked",
  internal: "vault.error.internal",
} as const satisfies Record<VaultErrorCode, I18nPath>;

export function vaultErrorText(
  code: VaultErrorCode | null,
  fallback: string | null = null,
): string | null {
  if (code === null) return fallback;
  return translateKey(ERROR_TEXT_KEYS[code]);
}

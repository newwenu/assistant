import type { VaultErrorCode, VaultErrorDto } from "./vault.types";
import { vaultErrorText } from "./vaultErrorText";

const VAULT_ERROR_CODES: readonly VaultErrorCode[] = [
  "locked",
  "invalidPassword",
  "notInitialized",
  "alreadyExists",
  "rateLimited",
  "crypto",
  "notFound",
  "entryCorrupted",
  "validation",
  "biometricUnavailable",
  "biometricCancelled",
  "biometricFailed",
  "biometricReset",
  "invalidKeyFormat",
  "invalidRecoveryKeyFormat",
  "invalidRecoveryKeyChecksum",
  "invalidRecoveryKeyOrMetaTampered",
  "vaultMetaIntegrityFailed",
  "unsupportedVaultVersion",
  "vaultVersionTooNew",
  "localDbKeyMismatch",
  "vaultRollbackDetected",
  "vaultManifestMismatch",
  "vaultCorrupted",
  "cancelled",
  "repoNotInitialized",
  "noRemoteConfigured",
  "noUpstream",
  "syncConflict",
  "gitUnavailable",
  "keyMismatch",
  "historyDecryptFailed",
  "internal",
];

export function isVaultErrorDto(value: unknown): value is VaultErrorDto {
  if (typeof value !== "object" || value === null) return false;
  const obj = value as Record<string, unknown>;
  return (
    typeof obj.code === "string" &&
    VAULT_ERROR_CODES.includes(obj.code as VaultErrorCode) &&
    typeof obj.message === "string" &&
    typeof obj.retryable === "boolean"
  );
}

export function vaultErrorCode(value: unknown): VaultErrorCode | null {
  return isVaultErrorDto(value) ? value.code : null;
}

export function vaultErrorMessage(value: unknown): string {
  if (!isVaultErrorDto(value)) return String(value);
  return vaultErrorText(value.code, value.message) ?? value.message;
}

export function needsRepositoryReset(code: VaultErrorCode | null): boolean {
  return code === "unsupportedVaultVersion";
}

export function needsDbRepair(code: VaultErrorCode | null): boolean {
  return code === "localDbKeyMismatch" || code === "vaultRollbackDetected";
}

export function needsAppUpgrade(code: VaultErrorCode | null): boolean {
  return code === "vaultVersionTooNew";
}

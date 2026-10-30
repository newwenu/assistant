import type { VaultErrorCode, VaultErrorDto } from "./vault.types";

const VAULT_ERROR_CODES: readonly VaultErrorCode[] = [
  "locked",
  "invalidPassword",
  "notInitialized",
  "alreadyExists",
  "rateLimited",
  "crypto",
  "notFound",
  "validation",
  "biometricUnavailable",
  "biometricCancelled",
  "biometricFailed",
  "biometricReset",
  "invalidKeyFormat",
  "invalidRecoveryKey",
  "recoveryKeyNotSet",
  "vaultMetaIntegrityFailed",
  "cancelled",
  "repoNotInitialized",
  "noRemoteConfigured",
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
  return isVaultErrorDto(value) ? value.message : String(value);
}

import type { VaultErrorCode, VaultErrorDto } from "../vault.types";
import { vaultErrorText } from "./vaultErrorText";

/**
 * 全部错误码的显式镜像表：`satisfies Record<VaultErrorCode, true>` 让生成契约
 * 新增错误码时在编译期报错，避免 `isVaultErrorDto` 静默漏判新码。
 */
const VAULT_ERROR_CODE_FLAGS = {
  locked: true,
  invalidPassword: true,
  notInitialized: true,
  alreadyExists: true,
  rateLimited: true,
  crypto: true,
  notFound: true,
  entryCorrupted: true,
  validation: true,
  biometricUnavailable: true,
  biometricCancelled: true,
  biometricFailed: true,
  biometricReset: true,
  invalidKeyFormat: true,
  invalidRecoveryKeyFormat: true,
  invalidRecoveryKeyChecksum: true,
  invalidRecoveryKeyOrMetaTampered: true,
  vaultMetaIntegrityFailed: true,
  unsupportedVaultVersion: true,
  vaultVersionTooNew: true,
  localDbKeyMismatch: true,
  vaultRollbackDetected: true,
  vaultManifestMismatch: true,
  vaultCorrupted: true,
  cancelled: true,
  repoNotInitialized: true,
  noRemoteConfigured: true,
  noUpstream: true,
  syncConflict: true,
  gitUnavailable: true,
  keyMismatch: true,
  historyDecryptFailed: true,
  vaultDekVersionAhead: true,
  vaultDekVersionBehind: true,
  repositoryContainsForbiddenFile: true,
  historyDekUnavailable: true,
  database: true,
  io: true,
  clipboard: true,
  alreadyUnlocked: true,
  internal: true,
} as const satisfies Record<VaultErrorCode, true>;

function isVaultErrorCode(value: string): value is VaultErrorCode {
  return Object.prototype.hasOwnProperty.call(VAULT_ERROR_CODE_FLAGS, value);
}

export function isVaultErrorDto(value: unknown): value is VaultErrorDto {
  if (typeof value !== "object" || value === null) return false;
  const obj = value as Record<string, unknown>;
  return (
    typeof obj.code === "string" &&
    isVaultErrorCode(obj.code) &&
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

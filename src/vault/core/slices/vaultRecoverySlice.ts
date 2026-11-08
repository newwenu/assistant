import { createLogger } from "@/services/logger";
import { vaultService } from "../vault.service";
import { vaultErrorCode, vaultErrorMessage } from "../vaultError";
import type {
  IntegrityResult,
  DisasterRecoveryResult,
  VaultErrorCode,
} from "../vault.types";

const log = createLogger("vaultStore");

const BIOMETRIC_CACHE_TTL_MS = 5_000;

export interface VaultRecoverySlice {
  _recoveryKeySetAt: number;
  recoveryKeySet: boolean;
  integrityResult: IntegrityResult | null;
  disasterRecoveryStep: 1 | 2 | 3 | null;
  disasterRecoveryResult: DisasterRecoveryResult | null;
  exportFilePath: string | null;

  generateRecoveryKey: () => Promise<string | null>;
  checkRecoveryKeySet: () => Promise<void>;
  verifyIntegrity: () => Promise<IntegrityResult | null>;
  startDisasterRecovery: () => void;
  cancelDisasterRecovery: () => void;
  setDisasterRecoveryStep: (step: 1 | 2 | 3) => void;
  disasterRecoveryExport: (
    gitUrl: string | null,
    recoveryKey: string,
    exportPath: string,
    commit?: string | null,
  ) => Promise<DisasterRecoveryResult | null>;
  exportVault: (format: "csv" | "json", path: string) => Promise<string | null>;
  importVault: (path: string, format: "csv" | "json") => Promise<number | null>;
}

type FullState = VaultRecoverySlice & {
  isLocked: boolean;
  loading: boolean;
  error: string | null;
  errorCode: VaultErrorCode | null;
  _biometricEnrolledAt: number;
  resetAutoLockTimer: () => void;
  loadEntries: () => Promise<void>;
};

export function createRecoverySlice(
  set: (
    partial: Partial<FullState> | ((state: FullState) => Partial<FullState>),
  ) => void,
  get: () => FullState,
): VaultRecoverySlice {
  return {
    _recoveryKeySetAt: 0,
    recoveryKeySet: false,
    integrityResult: null,
    disasterRecoveryStep: null,
    disasterRecoveryResult: null,
    exportFilePath: null,

    generateRecoveryKey: async () => {
      set({ loading: true, error: null, errorCode: null });
      try {
        const key = await vaultService.generateRecoveryKey();
        set({ recoveryKeySet: true, loading: false });
        return key;
      } catch (e) {
        log.error("generateRecoveryKey failed", e);
        set({
          error: vaultErrorMessage(e),
          errorCode: vaultErrorCode(e),
          loading: false,
        });
        return null;
      }
    },

    checkRecoveryKeySet: async () => {
      const now = Date.now();
      if (now - get()._recoveryKeySetAt < BIOMETRIC_CACHE_TTL_MS) return;
      try {
        const has = await vaultService.hasRecoveryKey();
        set({ recoveryKeySet: has, _recoveryKeySetAt: Date.now() });
      } catch (e) {
        log.error("checkRecoveryKeySet failed", e);
      }
    },

    verifyIntegrity: async () => {
      try {
        const result = await vaultService.verifyVaultMetaIntegrity();
        set({ integrityResult: result });
        return result;
      } catch (e) {
        log.error("verifyIntegrity failed", e);
        set({ error: vaultErrorMessage(e), errorCode: vaultErrorCode(e) });
        return null;
      }
    },

    startDisasterRecovery: () => {
      set({
        disasterRecoveryStep: 1,
        disasterRecoveryResult: null,
        error: null,
        errorCode: null,
      });
    },

    cancelDisasterRecovery: () => {
      set({ disasterRecoveryStep: null, disasterRecoveryResult: null });
    },

    setDisasterRecoveryStep: (step) => {
      set({ disasterRecoveryStep: step });
    },

    disasterRecoveryExport: async (gitUrl, recoveryKey, exportPath, commit) => {
      set({ loading: true, error: null, errorCode: null });
      try {
        const result = await vaultService.disasterRecoveryExport(
          gitUrl,
          recoveryKey,
          exportPath,
          commit,
        );
        set({
          disasterRecoveryStep: 3,
          disasterRecoveryResult: result,
          loading: false,
        });
        return result;
      } catch (e) {
        log.error("disasterRecoveryExport failed", e);
        set({
          error: vaultErrorMessage(e),
          errorCode: vaultErrorCode(e),
          loading: false,
        });
        return null;
      }
    },

    exportVault: async (format, path) => {
      set({ loading: true, error: null, errorCode: null });
      try {
        await vaultService.exportVaultToFile(format, path);
        set({ exportFilePath: path, loading: false });
        return path;
      } catch (e) {
        log.error("exportVault failed", e);
        set({
          error: vaultErrorMessage(e),
          errorCode: vaultErrorCode(e),
          loading: false,
        });
        return null;
      }
    },

    importVault: async (path, format) => {
      set({ loading: true, error: null, errorCode: null });
      try {
        const count = await vaultService.importVault(path, format);
        await get().loadEntries();
        set({ loading: false });
        return count;
      } catch (e) {
        log.error("importVault failed", e);
        set({
          error: vaultErrorMessage(e),
          errorCode: vaultErrorCode(e),
          loading: false,
        });
        return null;
      }
    },
  };
}

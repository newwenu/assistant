import { createLogger } from "@/services/logger";
import { vaultService } from "../vault.service";
import { vaultErrorCode, vaultErrorMessage } from "../vaultError";
import type {
  PaperErrorPosDto,
  RecoveryBackupDto,
  RecoveryExportDto,
  RecoveryKeyDto,
  VaultErrorCode,
} from "../vault.types";

const log = createLogger("vaultStore");

const RECOVERY_BACKUP_CACHE_TTL_MS = 5_000;

const EXPORT_CONFIRM_TOKEN = "EXPORT-PLAINTEXT";

export interface VaultRecoverySlice {
  _recoveryBackupAt: number;
  recoveryBackup: RecoveryBackupDto | null;
  exportFilePath: string | null;

  loadRecoveryBackup: () => Promise<void>;
  exportRecoveryKey: (masterPassword: string) => Promise<RecoveryKeyDto | null>;
  rotateDek: () => Promise<RecoveryKeyDto | null>;
  recoverFromPaper: (
    paperKey: string,
    gitUrl?: string,
  ) => Promise<RecoveryExportDto | null>;
  locatePaperError: (paperKey: string) => Promise<PaperErrorPosDto | null>;
  exportVault: (format: "csv" | "json", path: string) => Promise<string | null>;
  importVault: (path: string, format: "csv" | "json") => Promise<number | null>;
}

type FullState = VaultRecoverySlice & {
  isLocked: boolean;
  loading: boolean;
  error: string | null;
  errorCode: VaultErrorCode | null;
  biometricEnrolled: boolean;
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
    _recoveryBackupAt: 0,
    recoveryBackup: null,
    exportFilePath: null,

    loadRecoveryBackup: async () => {
      const now = Date.now();
      if (now - get()._recoveryBackupAt < RECOVERY_BACKUP_CACHE_TTL_MS) return;
      try {
        const backup = await vaultService.hasRecoveryBackup();
        set({ recoveryBackup: backup, _recoveryBackupAt: Date.now() });
      } catch (e) {
        log.error("loadRecoveryBackup failed", e);
      }
    },

    exportRecoveryKey: async (masterPassword) => {
      set({ loading: true, error: null, errorCode: null });
      try {
        const result = await vaultService.exportRecoveryKey(masterPassword);
        const backup = await vaultService.hasRecoveryBackup();
        set({
          loading: false,
          recoveryBackup: backup,
          _recoveryBackupAt: Date.now(),
        });
        return result;
      } catch (e) {
        log.error("exportRecoveryKey failed", e);
        set({
          error: vaultErrorMessage(e),
          errorCode: vaultErrorCode(e),
          loading: false,
        });
        return null;
      }
    },

    rotateDek: async () => {
      set({ loading: true, error: null, errorCode: null });
      try {
        const result = await vaultService.rotateDek();
        const backup = await vaultService.hasRecoveryBackup();
        set({
          loading: false,
          recoveryBackup: backup,
          _recoveryBackupAt: Date.now(),
          // rotate_dek 第 1 步已删除本机生物识别凭据，必须让用户重新注册（R8）。
          biometricEnrolled: false,
          _biometricEnrolledAt: 0,
        });
        await get().loadEntries();
        return result;
      } catch (e) {
        log.error("rotateDek failed", e);
        set({
          error: vaultErrorMessage(e),
          errorCode: vaultErrorCode(e),
          loading: false,
        });
        return null;
      }
    },

    recoverFromPaper: async (paperKey, gitUrl) => {
      set({ loading: true, error: null, errorCode: null });
      try {
        const result = await vaultService.recoverFromPaper(paperKey, gitUrl);
        set({ loading: false });
        return result;
      } catch (e) {
        log.error("recoverFromPaper failed", e);
        set({
          error: vaultErrorMessage(e),
          errorCode: vaultErrorCode(e),
          loading: false,
        });
        return null;
      }
    },

    locatePaperError: async (paperKey) => {
      try {
        return await vaultService.locatePaperError(paperKey);
      } catch (e) {
        log.error("locatePaperError failed", e);
        return null;
      }
    },

    exportVault: async (format, path) => {
      set({ loading: true, error: null, errorCode: null });
      try {
        const result = await vaultService.exportVaultToFile(
          format,
          path,
          EXPORT_CONFIRM_TOKEN,
        );
        set({ exportFilePath: result.path, loading: false });
        return result.path;
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

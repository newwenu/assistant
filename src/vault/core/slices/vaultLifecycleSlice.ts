import { createLogger } from "@/services/logger";
import { vaultService } from "../vault.service";
import { getConfValue } from "@/settings/core/useConfGroup";
import { vaultErrorCode, vaultErrorMessage } from "../vaultError";
import type {
  VaultEntrySummary,
  VaultEntryDetail,
  RevealedField,
  VaultErrorCode,
} from "../vault.types";

const log = createLogger("vaultStore");

const DEFAULT_AUTO_LOCK_MINUTES = 5;

function getAutoLockMs(): number {
  const minutes = getConfValue("vault")?.vaultAutoLockMinutes;
  return (minutes ?? DEFAULT_AUTO_LOCK_MINUTES) * 60 * 1000;
}

export interface VaultLifecycleSlice {
  isLocked: boolean;
  isInitialized: boolean;
  loading: boolean;
  error: string | null;
  errorCode: VaultErrorCode | null;
  autoLockTimerId: ReturnType<typeof setTimeout> | null;

  init: (masterPassword: string) => Promise<void>;
  unlock: (masterPassword: string) => Promise<void>;
  lock: () => Promise<void>;
  changeMasterPassword: (current: string, newPassword: string) => Promise<void>;
  checkStatus: () => Promise<void>;
  clearError: () => void;
  resetAutoLockTimer: () => void;
}

type FullState = VaultLifecycleSlice & {
  entries: VaultEntrySummary[];
  selectedEntry: VaultEntryDetail | null;
  revealedField: RevealedField | null;
  revealTimerId: ReturnType<typeof setTimeout> | null;
  clearRevealedField: () => void;
  _biometricEnrolledAt: number;
  _recoveryKeySetAt: number;
};

export function createLifecycleSlice(
  set: (
    partial: Partial<FullState> | ((state: FullState) => Partial<FullState>),
  ) => void,
  get: () => FullState,
): VaultLifecycleSlice {
  return {
    isLocked: true,
    isInitialized: false,
    loading: false,
    error: null,
    errorCode: null,
    autoLockTimerId: null,

    init: async (masterPassword) => {
      set({ loading: true, error: null });
      try {
        await vaultService.init(masterPassword);
        set({
          isLocked: false,
          isInitialized: true,
          loading: false,
          _biometricEnrolledAt: 0,
          _recoveryKeySetAt: 0,
        });
        get().resetAutoLockTimer();
      } catch (e) {
        log.error("init failed", e);
        set({
          error: vaultErrorMessage(e),
          errorCode: vaultErrorCode(e),
          loading: false,
        });
      }
    },

    unlock: async (masterPassword) => {
      set({ loading: true, error: null, errorCode: null });
      try {
        await vaultService.unlock(masterPassword);
        set({
          isLocked: false,
          loading: false,
          _biometricEnrolledAt: 0,
          _recoveryKeySetAt: 0,
        });
        get().resetAutoLockTimer();
      } catch (e) {
        log.error("unlock failed", e);
        set({
          error: vaultErrorMessage(e),
          errorCode: vaultErrorCode(e),
          loading: false,
        });
      }
    },

    lock: async () => {
      try {
        await vaultService.lock();
        set({
          isLocked: true,
          entries: [],
          selectedEntry: null,
          _biometricEnrolledAt: 0,
          _recoveryKeySetAt: 0,
        });
        get().clearRevealedField();
        const { autoLockTimerId } = get();
        if (autoLockTimerId) clearTimeout(autoLockTimerId);
      } catch (e) {
        log.error("lock failed", e);
      }
    },

    changeMasterPassword: async (current, newPassword) => {
      set({ loading: true, error: null });
      try {
        await vaultService.changeMasterPassword(current, newPassword);
        set({
          loading: false,
          _biometricEnrolledAt: 0,
          _recoveryKeySetAt: 0,
        });
      } catch (e) {
        log.error("changeMasterPassword failed", e);
        set({
          error: vaultErrorMessage(e),
          errorCode: vaultErrorCode(e),
          loading: false,
        });
        throw e;
      }
    },

    checkStatus: async () => {
      try {
        const [locked, initialized] = await Promise.all([
          vaultService.isLocked(),
          vaultService.isInitialized(),
        ]);
        set({ isLocked: locked, isInitialized: initialized });
        if (!locked) get().resetAutoLockTimer();
      } catch (e) {
        log.error("checkStatus failed", e);
      }
    },

    clearError: () => set({ error: null, errorCode: null }),

    resetAutoLockTimer: () => {
      const { autoLockTimerId } = get();
      if (autoLockTimerId) clearTimeout(autoLockTimerId);
      const id = setTimeout(() => {
        void get().lock();
      }, getAutoLockMs());
      set({ autoLockTimerId: id });
    },
  };
}

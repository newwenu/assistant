import { createLogger } from "@/services/logger";
import { vaultService } from "../vault.service";
import { useSettingsStore } from "@/stores/settingsStore";
import type {
  VaultEntrySummary,
  VaultEntryDetail,
  RevealedField,
} from "../vault.types";

const log = createLogger("vaultStore");

const DEFAULT_AUTO_LOCK_MINUTES = 5;

function getAutoLockMs(): number {
  const minutes = useSettingsStore.getState().appConf?.vaultAutoLockMinutes;
  return (minutes ?? DEFAULT_AUTO_LOCK_MINUTES) * 60 * 1000;
}

export interface VaultLifecycleSlice {
  isLocked: boolean;
  isInitialized: boolean;
  loading: boolean;
  error: string | null;
  autoLockTimerId: ReturnType<typeof setTimeout> | null;

  init: (masterPassword: string) => Promise<void>;
  unlock: (masterPassword: string) => Promise<boolean>;
  lock: () => Promise<void>;
  checkStatus: () => Promise<void>;
  clearError: () => void;
  resetAutoLockTimer: () => void;
}

type FullState = VaultLifecycleSlice & {
  entries: VaultEntrySummary[];
  selectedEntry: VaultEntryDetail | null;
  revealedField: RevealedField | null;
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
        set({ error: String(e), loading: false });
      }
    },

    unlock: async (masterPassword) => {
      set({ loading: true, error: null });
      try {
        const ok = await vaultService.unlock(masterPassword);
        if (ok) {
          set({
            isLocked: false,
            loading: false,
            _biometricEnrolledAt: 0,
            _recoveryKeySetAt: 0,
          });
          get().resetAutoLockTimer();
        } else {
          set({ error: "Invalid password", loading: false });
        }
        return ok;
      } catch (e) {
        log.error("unlock failed", e);
        set({ error: String(e), loading: false });
        return false;
      }
    },

    lock: async () => {
      try {
        await vaultService.lock();
        set({
          isLocked: true,
          entries: [],
          selectedEntry: null,
          revealedField: null,
          _biometricEnrolledAt: 0,
          _recoveryKeySetAt: 0,
        });
        const { autoLockTimerId } = get();
        if (autoLockTimerId) clearTimeout(autoLockTimerId);
      } catch (e) {
        log.error("lock failed", e);
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

    clearError: () => set({ error: null }),

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

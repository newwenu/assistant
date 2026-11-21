import { createLogger } from "@/services/system/logger";
import { vaultService } from "../vault.service";
import { getConfValue } from "@/settings/core/runtime/useConfGroup";
import { vaultErrorCode, vaultErrorMessage } from "../display/vaultError";
import type {
  VaultEntrySummary,
  VaultEntryDetail,
  RevealedField,
  VaultErrorCode,
  LockReason,
} from "../vault.types";

const log = createLogger("vaultStore");

const DEFAULT_AUTO_LOCK_MINUTES = 5;
const AUTO_LOCK_RESET_THROTTLE_MS = 500;

let autoLockTimer: ReturnType<typeof setTimeout> | null = null;
let lastAutoLockResetAt = 0;

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
  /** 自动锁定截止时间（unix 毫秒）；null 表示未计时或已锁定。 */
  autoLockDeadline: number | null;

  init: (masterPassword: string) => Promise<void>;
  unlock: (masterPassword: string) => Promise<void>;
  lock: (reason?: LockReason) => Promise<void>;
  changeMasterPassword: (current: string, newPassword: string) => Promise<void>;
  resetRepository: (confirmToken: string) => Promise<void>;
  rebuildLocalDb: (masterPassword: string) => Promise<void>;
  checkStatus: () => Promise<void>;
  clearError: () => void;
  resetAutoLockTimer: (force?: boolean) => void;
  clearAutoLockTimer: () => void;
}

type FullState = VaultLifecycleSlice & {
  entries: VaultEntrySummary[];
  totalEntryCount: number;
  selectedEntry: VaultEntryDetail | null;
  revealedField: RevealedField | null;
  revealTimerId: ReturnType<typeof setTimeout> | null;
  clearRevealedField: () => void;
  searchQuery: string;
  categoryFilter: import("../vault.types").VaultCategory | null;
  tagFilter: string | null;
  onlyLeaked: boolean;
  sortBy: import("./vaultEntriesSlice").VaultSortBy;
  hibpLeaks: Record<string, number>;
  _biometricEnrolledAt: number;
  _recoveryBackupAt: number;
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
    autoLockDeadline: null,

    init: async (masterPassword) => {
      set({ loading: true, error: null });
      try {
        await vaultService.init(masterPassword);
        set({
          isLocked: false,
          isInitialized: true,
          loading: false,
          _biometricEnrolledAt: 0,
          _recoveryBackupAt: 0,
        });
        get().resetAutoLockTimer(true);
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
          _recoveryBackupAt: 0,
        });
        get().resetAutoLockTimer(true);
      } catch (e) {
        log.error("unlock failed", e);
        set({
          error: vaultErrorMessage(e),
          errorCode: vaultErrorCode(e),
          loading: false,
        });
      }
    },

    lock: async (reason) => {
      get().clearAutoLockTimer();
      try {
        await vaultService.lock(reason);
        set({
          isLocked: true,
          entries: [],
          totalEntryCount: 0,
          selectedEntry: null,
          searchQuery: "",
          categoryFilter: null,
          tagFilter: null,
          onlyLeaked: false,
          sortBy: "recent",
          hibpLeaks: {},
          _biometricEnrolledAt: 0,
          _recoveryBackupAt: 0,
        });
        get().clearRevealedField();
      } catch (e) {
        log.error("lock failed", e);
        set({
          error: vaultErrorMessage(e),
          errorCode: vaultErrorCode(e),
        });
        get().resetAutoLockTimer(true);
        throw e;
      }
    },

    changeMasterPassword: async (current, newPassword) => {
      set({ loading: true, error: null });
      try {
        await vaultService.changeMasterPassword(current, newPassword);
        set({
          loading: false,
          _biometricEnrolledAt: 0,
          _recoveryBackupAt: 0,
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

    resetRepository: async (confirmToken) => {
      get().clearAutoLockTimer();
      set({ loading: true, error: null, errorCode: null });
      try {
        await vaultService.resetRepository(confirmToken);
        set({
          isLocked: true,
          isInitialized: false,
          entries: [],
          totalEntryCount: 0,
          selectedEntry: null,
          searchQuery: "",
          categoryFilter: null,
          tagFilter: null,
          onlyLeaked: false,
          sortBy: "recent",
          hibpLeaks: {},
          _biometricEnrolledAt: 0,
          _recoveryBackupAt: 0,
          error: null,
          errorCode: null,
          loading: false,
        });
        get().clearRevealedField();
      } catch (e) {
        log.error("resetRepository failed", e);
        set({
          error: vaultErrorMessage(e),
          errorCode: vaultErrorCode(e),
          loading: false,
        });
        throw e;
      }
    },

    rebuildLocalDb: async (masterPassword) => {
      set({ loading: true, error: null, errorCode: null });
      try {
        await vaultService.rebuildLocalDb(masterPassword);
        set({
          isLocked: false,
          isInitialized: true,
          loading: false,
          _biometricEnrolledAt: 0,
          _recoveryBackupAt: 0,
        });
        get().resetAutoLockTimer(true);
      } catch (e) {
        log.error("rebuildLocalDb failed", e);
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
        if (!locked) get().resetAutoLockTimer(true);
      } catch (e) {
        log.error("checkStatus failed", e);
      }
    },

    clearError: () => set({ error: null, errorCode: null }),

    clearAutoLockTimer: () => {
      if (autoLockTimer) {
        clearTimeout(autoLockTimer);
        autoLockTimer = null;
      }
      lastAutoLockResetAt = 0;
      set({ autoLockDeadline: null });
    },

    resetAutoLockTimer: (force = false) => {
      if (get().isLocked) return;
      const now = Date.now();
      if (!force && now - lastAutoLockResetAt < AUTO_LOCK_RESET_THROTTLE_MS) {
        return;
      }
      lastAutoLockResetAt = now;
      if (autoLockTimer) clearTimeout(autoLockTimer);
      autoLockTimer = null;
      const autoLockMs = getAutoLockMs();
      if (autoLockMs <= 0) {
        set({ autoLockDeadline: null });
        return;
      }
      const deadline = now + autoLockMs;
      autoLockTimer = setTimeout(() => {
        autoLockTimer = null;
        void get()
          .lock("autoLock")
          .catch((e: unknown) => log.warn("autoLock failed", e));
      }, deadline - now);
      set({ autoLockDeadline: deadline });
    },
  };
}

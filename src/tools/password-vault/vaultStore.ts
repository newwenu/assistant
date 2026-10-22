import { create } from "zustand";
import { createLogger } from "@/services/logger";
import type {
  VaultEntrySummary,
  VaultEntryDetail,
  RevealResult,
  VaultEntryCreate,
  VaultEntryUpdate,
  VaultCategory,
  TotpResult,
  StrengthResult,
} from "./vault.service";
import { vaultService } from "./vault.service";
import { useSettingsStore } from "@/stores/settingsStore";

const log = createLogger("vaultStore");

interface RevealedField {
  value: string;
  expiresAt: number;
  kind: "password" | "notes";
}

interface VaultState {
  isLocked: boolean;
  isInitialized: boolean;
  loading: boolean;
  error: string | null;
  entries: VaultEntrySummary[];
  selectedEntry: VaultEntryDetail | null;
  searchQuery: string;
  categoryFilter: VaultCategory | null;
  tagFilter: string | null;
  revealedField: RevealedField | null;
  autoLockTimerId: ReturnType<typeof setTimeout> | null;
  totpCode: string | null;
  totpTimeRemaining: number;
  totpTimerId: ReturnType<typeof setInterval> | null;
  hibpLeaks: Record<string, number>;
  hibpAuditing: boolean;
  biometricAvailable: boolean;
  biometricEnrolled: boolean;

  init: (masterPassword: string) => Promise<void>;
  unlock: (masterPassword: string) => Promise<boolean>;
  lock: () => Promise<void>;
  checkStatus: () => Promise<void>;
  loadEntries: () => Promise<void>;
  selectEntry: (id: string) => Promise<void>;
  clearSelectedEntry: () => void;
  createEntry: (entry: VaultEntryCreate) => Promise<string>;
  updateEntry: (id: string, entry: VaultEntryUpdate) => Promise<void>;
  deleteEntry: (id: string) => Promise<void>;
  copyPassword: (id: string) => Promise<void>;
  copyField: (id: string, fieldName: string) => Promise<void>;
  revealPassword: (id: string) => Promise<void>;
  revealNotes: (id: string) => Promise<void>;
  clearRevealedField: () => void;
  setSearchQuery: (query: string) => void;
  setCategoryFilter: (category: VaultCategory | null) => void;
  setTagFilter: (tag: string | null) => void;
  resetAutoLockTimer: () => void;
  clearError: () => void;
  generateTotp: (
    id: string,
    period?: number,
    digits?: number,
    algorithm?: string,
  ) => Promise<void>;
  stopTotpTimer: () => void;
  hibpCheck: (id: string) => Promise<number>;
  hibpAudit: () => Promise<void>;
  checkMasterPasswordStrength: (
    password: string,
  ) => Promise<StrengthResult | null>;
  checkBiometricAvailable: () => Promise<void>;
  checkBiometricEnrolled: () => Promise<void>;
  unlockBiometric: () => Promise<boolean>;
  enrollBiometric: (masterPassword: string) => Promise<void>;
  disenrollBiometric: () => Promise<void>;
}

const DEFAULT_AUTO_LOCK_MINUTES = 5;

function getAutoLockMs(): number {
  const minutes = useSettingsStore.getState().appConf?.vaultAutoLockMinutes;
  return (minutes ?? DEFAULT_AUTO_LOCK_MINUTES) * 60 * 1000;
}

export const useVaultStore = create<VaultState>((set, get) => ({
  isLocked: true,
  isInitialized: false,
  loading: false,
  error: null,
  entries: [],
  selectedEntry: null,
  searchQuery: "",
  categoryFilter: null,
  tagFilter: null,
  revealedField: null,
  autoLockTimerId: null,
  totpCode: null,
  totpTimeRemaining: 0,
  totpTimerId: null,
  hibpLeaks: {},
  hibpAuditing: false,
  biometricAvailable: false,
  biometricEnrolled: false,

  init: async (masterPassword) => {
    set({ loading: true, error: null });
    try {
      await vaultService.init(masterPassword);
      set({ isLocked: false, isInitialized: true, loading: false });
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
        set({ isLocked: false, loading: false });
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

  loadEntries: async () => {
    set({ loading: true, error: null });
    try {
      const { categoryFilter, searchQuery, tagFilter } = get();
      const entries = await vaultService.listEntries(
        categoryFilter ?? undefined,
        searchQuery || undefined,
        tagFilter ?? undefined,
      );
      set({ entries, loading: false });
    } catch (e) {
      log.error("loadEntries failed", e);
      set({ error: String(e), loading: false });
    }
  },

  selectEntry: async (id) => {
    try {
      const detail = await vaultService.getEntry(id);
      set({ selectedEntry: detail });
    } catch (e) {
      log.error("selectEntry failed", e);
      set({ error: String(e) });
    }
  },

  clearSelectedEntry: () => set({ selectedEntry: null }),

  createEntry: async (entry) => {
    set({ loading: true, error: null });
    try {
      const id = await vaultService.createEntry(entry);
      await get().loadEntries();
      set({ loading: false });
      return id;
    } catch (e) {
      log.error("createEntry failed", e);
      set({ error: String(e), loading: false });
      throw e;
    }
  },

  updateEntry: async (id, entry) => {
    set({ loading: true, error: null });
    try {
      await vaultService.updateEntry(id, entry);
      await get().loadEntries();
      set({ loading: false });
    } catch (e) {
      log.error("updateEntry failed", e);
      set({ error: String(e), loading: false });
    }
  },

  deleteEntry: async (id) => {
    set({ loading: true, error: null });
    try {
      await vaultService.deleteEntry(id);
      const { selectedEntry } = get();
      if (selectedEntry?.id === id) set({ selectedEntry: null });
      await get().loadEntries();
      set({ loading: false });
    } catch (e) {
      log.error("deleteEntry failed", e);
      set({ error: String(e), loading: false });
    }
  },

  copyPassword: async (id) => {
    try {
      await vaultService.copyPassword(id);
    } catch (e) {
      log.error("copyPassword failed", e);
      set({ error: String(e) });
    }
  },

  copyField: async (id, fieldName) => {
    try {
      await vaultService.copyField(id, fieldName);
    } catch (e) {
      log.error("copyField failed", e);
      set({ error: String(e) });
    }
  },

  revealPassword: async (id) => {
    try {
      const result: RevealResult = await vaultService.revealPassword(id);
      set({
        revealedField: {
          value: result.value,
          expiresAt: result.expiresAt,
          kind: "password",
        },
      });
      const delay = Math.max(result.expiresAt - Date.now(), 0);
      setTimeout(() => {
        const current = get().revealedField;
        if (current?.kind === "password") get().clearRevealedField();
      }, delay);
    } catch (e) {
      log.error("revealPassword failed", e);
      set({ error: String(e) });
    }
  },

  revealNotes: async (id) => {
    try {
      const result: RevealResult = await vaultService.revealNotes(id);
      set({
        revealedField: {
          value: result.value,
          expiresAt: result.expiresAt,
          kind: "notes",
        },
      });
      const delay = Math.max(result.expiresAt - Date.now(), 0);
      setTimeout(() => {
        const current = get().revealedField;
        if (current?.kind === "notes") get().clearRevealedField();
      }, delay);
    } catch (e) {
      log.error("revealNotes failed", e);
      set({ error: String(e) });
    }
  },

  clearRevealedField: () => set({ revealedField: null }),

  setSearchQuery: (query) => {
    set({ searchQuery: query });
  },

  setCategoryFilter: (category) => {
    set({ categoryFilter: category });
  },

  setTagFilter: (tag) => {
    set({ tagFilter: tag });
  },

  resetAutoLockTimer: () => {
    const { autoLockTimerId } = get();
    if (autoLockTimerId) clearTimeout(autoLockTimerId);
    const id = setTimeout(() => {
      void get().lock();
    }, getAutoLockMs());
    set({ autoLockTimerId: id });
  },

  clearError: () => set({ error: null }),

  generateTotp: async (id, period, digits, algorithm) => {
    try {
      const result: TotpResult = await vaultService.totpGenerate(
        id,
        period,
        digits,
        algorithm,
      );
      set({ totpCode: result.code, totpTimeRemaining: result.timeRemaining });

      const { totpTimerId } = get();
      if (totpTimerId) clearInterval(totpTimerId);

      const timerId = setInterval(() => {
        const remaining = get().totpTimeRemaining - 1;
        if (remaining <= 0) {
          void get().generateTotp(id, period, digits, algorithm);
        } else {
          set({ totpTimeRemaining: remaining });
        }
      }, 1000);
      set({ totpTimerId: timerId });
    } catch (e) {
      log.error("generateTotp failed", e);
      set({ totpCode: null, totpTimeRemaining: 0 });
    }
  },

  stopTotpTimer: () => {
    const { totpTimerId } = get();
    if (totpTimerId) clearInterval(totpTimerId);
    set({ totpCode: null, totpTimeRemaining: 0, totpTimerId: null });
  },

  hibpCheck: async (id) => {
    try {
      const count = await vaultService.hibpCheck(id);
      set((state) => ({
        hibpLeaks: { ...state.hibpLeaks, [id]: count },
      }));
      return count;
    } catch (e) {
      log.error("hibpCheck failed", e);
      return 0;
    }
  },

  hibpAudit: async () => {
    set({ hibpAuditing: true });
    try {
      const leaks = await vaultService.hibpAudit();
      const leakMap: Record<string, number> = {};
      for (const [id, count] of leaks) {
        leakMap[id] = count;
      }
      set({ hibpLeaks: leakMap, hibpAuditing: false });
    } catch (e) {
      log.error("hibpAudit failed", e);
      set({ hibpAuditing: false });
    }
  },

  checkMasterPasswordStrength: async (password) => {
    try {
      return await vaultService.checkMasterPasswordStrength(password);
    } catch (e) {
      log.error("checkMasterPasswordStrength failed", e);
      return null;
    }
  },

  checkBiometricAvailable: async () => {
    try {
      const available = await vaultService.isBiometricAvailable();
      set({ biometricAvailable: available });
    } catch (e) {
      log.error("checkBiometricAvailable failed", e);
      set({ biometricAvailable: false });
    }
  },

  checkBiometricEnrolled: async () => {
    try {
      const enrolled = await vaultService.isBiometricEnrolled();
      set({ biometricEnrolled: enrolled });
    } catch (e) {
      log.error("checkBiometricEnrolled failed", e);
      set({ biometricEnrolled: false });
    }
  },

  unlockBiometric: async () => {
    set({ loading: true, error: null });
    try {
      const ok = await vaultService.unlockBiometric();
      if (ok) {
        set({ isLocked: false, loading: false });
        get().resetAutoLockTimer();
      } else {
        set({ loading: false });
      }
      return ok;
    } catch (e) {
      const msg = String(e);
      if (msg.includes("NotFound") || msg.includes("Crypto")) {
        try {
          await vaultService.disenrollBiometric();
          set({ biometricEnrolled: false });
        } catch {}
      }
      log.error("unlockBiometric failed", e);
      set({ error: msg, loading: false });
      return false;
    }
  },

  enrollBiometric: async (masterPassword) => {
    set({ loading: true, error: null });
    try {
      await vaultService.enrollBiometric(masterPassword);
      set({ biometricEnrolled: true, loading: false });
    } catch (e) {
      log.error("enrollBiometric failed", e);
      set({ error: String(e), loading: false });
    }
  },

  disenrollBiometric: async () => {
    try {
      await vaultService.disenrollBiometric();
      set({ biometricEnrolled: false });
    } catch (e) {
      log.error("disenrollBiometric failed", e);
    }
  },
}));
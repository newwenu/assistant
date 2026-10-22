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
  SyncResult,
  SyncConfig,
  Readyness,
  ConflictResolution,
  SshKeyInfo,
  IntegrityResult,
  DisasterRecoveryResult,
} from "./vault.service";
import { vaultService } from "./vault.service";
import { useSettingsStore } from "@/stores/settingsStore";

const log = createLogger("vaultStore");

const BIOMETRIC_CACHE_TTL_MS = 5_000;

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
  _biometricAvailableAt: number;
  _biometricEnrolledAt: number;
  _recoveryKeySetAt: number;
  syncReadyness: Readyness | null;
  syncResult: SyncResult | null;
  syncConfig: SyncConfig | null;
  syncing: boolean;
  sshAgentKeys: SshKeyInfo[];
  sshAgentRunning: boolean;
  recoveryKeySet: boolean;
  integrityResult: IntegrityResult | null;
  disasterRecoveryStep: 1 | 2 | 3 | null;
  disasterRecoveryResult: DisasterRecoveryResult | null;
  exportFilePath: string | null;

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
  copyTotp: (
    id: string,
    period?: number,
    digits?: number,
    algorithm?: string,
  ) => Promise<void>;
  stopTotpTimer: () => void;
  totpParseUri: (
    uri: string,
  ) => Promise<import("./vault.service").ParsedOtpAuth>;
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
  syncPull: () => Promise<void>;
  syncPush: () => Promise<void>;
  syncNow: () => Promise<void>;
  syncForcePush: () => Promise<void>;
  loadSyncConfig: () => Promise<void>;
  saveSyncConfig: (config: SyncConfig) => Promise<void>;
  loadSyncReadyness: () => Promise<void>;
  resolveConflict: (
    path: string,
    resolution: ConflictResolution,
  ) => Promise<void>;
  gitInit: () => Promise<void>;
  gitClone: (url: string) => Promise<void>;
  gitAddRemote: (remote: string, url: string) => Promise<void>;
  gitSetRemote: (remote: string, url: string) => Promise<void>;
  sshAddToAgent: (id: string, lifetime?: number) => Promise<string | null>;
  sshRemoveFromAgent: (fingerprint: string) => Promise<void>;
  sshListAgent: () => Promise<void>;
  sshAutoAddAll: () => Promise<string[]>;
  sshCheckAgentRunning: () => Promise<void>;
  generateRecoveryKey: () => Promise<string | null>;
  unlockWithRecoveryKey: (recoveryKey: string) => Promise<boolean>;
  checkRecoveryKeySet: () => Promise<void>;
  verifyIntegrity: () => Promise<IntegrityResult | null>;
  startDisasterRecovery: () => void;
  cancelDisasterRecovery: () => void;
  setDisasterRecoveryStep: (step: 1 | 2 | 3) => void;
  disasterRecoveryExport: (
    gitUrl: string | null,
    recoveryKey: string,
    exportPath: string,
  ) => Promise<DisasterRecoveryResult | null>;
  exportVault: (format: "csv" | "json") => Promise<string | null>;
  importVault: (path: string, format: "csv" | "json") => Promise<number | null>;
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
  _biometricAvailableAt: 0,
  _biometricEnrolledAt: 0,
  _recoveryKeySetAt: 0,
  syncReadyness: null,
  syncResult: null,
  syncConfig: null,
  syncing: false,
  sshAgentKeys: [],
  sshAgentRunning: false,
  recoveryKeySet: false,
  integrityResult: null,
  disasterRecoveryStep: null,
  disasterRecoveryResult: null,
  exportFilePath: null,

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

  copyTotp: async (id, period, digits, algorithm) => {
    try {
      await vaultService.totpCopy(id, period, digits, algorithm);
    } catch (e) {
      log.error("copyTotp failed", e);
      set({ error: String(e) });
    }
  },

  stopTotpTimer: () => {
    const { totpTimerId } = get();
    if (totpTimerId) clearInterval(totpTimerId);
    set({ totpCode: null, totpTimeRemaining: 0, totpTimerId: null });
  },

  totpParseUri: async (uri: string) => {
    return vaultService.totpParseUri(uri);
  },

  queryAuditLog: async (limit?: number) => {
    return vaultService.queryAuditLog(limit);
  },

  showUnlockWindow: async () => {
    await vaultService.showUnlockWindow();
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
    const now = Date.now();
    if (now - get()._biometricAvailableAt < BIOMETRIC_CACHE_TTL_MS) return;
    try {
      const available = await vaultService.isBiometricAvailable();
      set({ biometricAvailable: available, _biometricAvailableAt: Date.now() });
      log.debug(`Biometric available: ${available}`);
    } catch (e) {
      log.error("checkBiometricAvailable failed", e);
      set({ biometricAvailable: false, _biometricAvailableAt: Date.now() });
    }
  },

  checkBiometricEnrolled: async () => {
    const now = Date.now();
    if (now - get()._biometricEnrolledAt < BIOMETRIC_CACHE_TTL_MS) return;
    try {
      const enrolled = await vaultService.isBiometricEnrolled();
      set({ biometricEnrolled: enrolled, _biometricEnrolledAt: Date.now() });
      log.debug(`Biometric enrolled: ${enrolled}`);
    } catch (e) {
      log.error("checkBiometricEnrolled failed", e);
      set({ biometricEnrolled: false, _biometricEnrolledAt: Date.now() });
    }
  },

  unlockBiometric: async () => {
    set({ loading: true, error: null });
    try {
      const ok = await vaultService.unlockBiometric();
      if (ok) {
        set({ isLocked: false, loading: false, _biometricEnrolledAt: 0 });
        get().resetAutoLockTimer();
        log.info("Vault unlocked via biometric");
      } else {
        set({ loading: false });
        log.warn("Biometric unlock cancelled or failed");
      }
      return ok;
    } catch (e) {
      const msg = String(e);
      if (msg.includes("NotFound") || msg.includes("Crypto")) {
        try {
          await vaultService.disenrollBiometric();
          set({ biometricEnrolled: false, _biometricEnrolledAt: 0 });
        } catch {
          log.warn(
            "disenrollBiometric also failed during unlockBiometric cleanup",
          );
        }
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
      set({ biometricEnrolled: true, loading: false, _biometricEnrolledAt: 0 });
      log.info("Biometric enrolled successfully");
    } catch (e) {
      log.error("enrollBiometric failed", e);
      set({ error: String(e), loading: false });
    }
  },

  disenrollBiometric: async () => {
    try {
      await vaultService.disenrollBiometric();
      set({ biometricEnrolled: false, _biometricEnrolledAt: 0 });
      log.info("Biometric disenrolled");
    } catch (e) {
      log.error("disenrollBiometric failed", e);
    }
  },

  syncPull: async () => {
    set({ syncing: true, error: null });
    try {
      const result = await vaultService.syncPull();
      set({ syncResult: result, syncing: false });
    } catch (e) {
      log.error("syncPull failed", e);
      set({ error: String(e), syncing: false });
    }
  },

  syncPush: async () => {
    set({ syncing: true, error: null });
    try {
      const result = await vaultService.syncPush();
      set({ syncResult: result, syncing: false });
    } catch (e) {
      log.error("syncPush failed", e);
      set({ error: String(e), syncing: false });
    }
  },

  syncNow: async () => {
    set({ syncing: true, error: null });
    try {
      const result = await vaultService.syncNow();
      set({ syncResult: result, syncing: false });
      await get().loadEntries();
      await get().loadSyncReadyness();
    } catch (e) {
      log.error("syncNow failed", e);
      set({ error: String(e), syncing: false });
    }
  },

  syncForcePush: async () => {
    set({ syncing: true, error: null });
    try {
      const result = await vaultService.syncForcePush();
      set({ syncResult: result, syncing: false });
    } catch (e) {
      log.error("syncForcePush failed", e);
      set({ error: String(e), syncing: false });
    }
  },

  loadSyncConfig: async () => {
    try {
      const config = await vaultService.getSyncConfig();
      set({ syncConfig: config });
    } catch (e) {
      log.error("loadSyncConfig failed", e);
    }
  },

  saveSyncConfig: async (config) => {
    try {
      await vaultService.setSyncConfig(config);
      set({ syncConfig: config });
    } catch (e) {
      log.error("saveSyncConfig failed", e);
      set({ error: String(e) });
    }
  },

  loadSyncReadyness: async () => {
    try {
      const readyness = await vaultService.syncReadyness();
      set({ syncReadyness: readyness });
    } catch (e) {
      log.error("loadSyncReadyness failed", e);
    }
  },

  resolveConflict: async (path, resolution) => {
    try {
      await vaultService.resolveConflict(path, resolution);
      await get().loadSyncReadyness();
    } catch (e) {
      log.error("resolveConflict failed", e);
      set({ error: String(e) });
    }
  },

  gitInit: async () => {
    set({ loading: true, error: null });
    try {
      await vaultService.gitInit();
      await get().loadSyncReadyness();
      set({ loading: false });
    } catch (e) {
      log.error("gitInit failed", e);
      set({ error: String(e), loading: false });
    }
  },

  gitClone: async (url) => {
    set({ loading: true, error: null });
    try {
      await vaultService.gitClone(url);
      await get().loadSyncReadyness();
      set({ loading: false });
    } catch (e) {
      log.error("gitClone failed", e);
      set({ error: String(e), loading: false });
    }
  },

  gitAddRemote: async (remote, url) => {
    try {
      await vaultService.gitAddRemote(remote, url);
      await get().loadSyncConfig();
    } catch (e) {
      log.error("gitAddRemote failed", e);
      set({ error: String(e) });
    }
  },

  gitSetRemote: async (remote, url) => {
    try {
      await vaultService.gitSetRemote(remote, url);
      await get().loadSyncConfig();
    } catch (e) {
      log.error("gitSetRemote failed", e);
      set({ error: String(e) });
    }
  },

  sshAddToAgent: async (id, lifetime) => {
    try {
      const fingerprint = await vaultService.sshAddToAgent(id, lifetime);
      await get().sshListAgent();
      return fingerprint;
    } catch (e) {
      log.error("sshAddToAgent failed", e);
      set({ error: String(e) });
      return null;
    }
  },

  sshRemoveFromAgent: async (fingerprint) => {
    try {
      await vaultService.sshRemoveFromAgent(fingerprint);
      await get().sshListAgent();
    } catch (e) {
      log.error("sshRemoveFromAgent failed", e);
      set({ error: String(e) });
    }
  },

  sshListAgent: async () => {
    try {
      const keys = await vaultService.sshListAgent();
      set({ sshAgentKeys: keys });
    } catch (e) {
      log.error("sshListAgent failed", e);
    }
  },

  sshAutoAddAll: async () => {
    try {
      const added = await vaultService.sshAutoAddAll();
      await get().sshListAgent();
      return added;
    } catch (e) {
      log.error("sshAutoAddAll failed", e);
      set({ error: String(e) });
      return [];
    }
  },

  sshCheckAgentRunning: async () => {
    try {
      const running = await vaultService.sshIsAgentRunning();
      set({ sshAgentRunning: running });
    } catch (e) {
      log.error("sshCheckAgentRunning failed", e);
    }
  },

  generateRecoveryKey: async () => {
    set({ loading: true, error: null });
    try {
      const key = await vaultService.generateRecoveryKey();
      set({ recoveryKeySet: true, loading: false });
      return key;
    } catch (e) {
      log.error("generateRecoveryKey failed", e);
      set({ error: String(e), loading: false });
      return null;
    }
  },

  unlockWithRecoveryKey: async (recoveryKey) => {
    set({ loading: true, error: null });
    try {
      const ok = await vaultService.unlockWithRecoveryKey(recoveryKey);
      if (ok) {
        set({
          isLocked: false,
          loading: false,
          _biometricEnrolledAt: 0,
          _recoveryKeySetAt: 0,
        });
        get().resetAutoLockTimer();
      } else {
        set({ error: "Invalid recovery key", loading: false });
      }
      return ok;
    } catch (e) {
      log.error("unlockWithRecoveryKey failed", e);
      set({ error: String(e), loading: false });
      return false;
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
      set({ error: String(e) });
      return null;
    }
  },

  startDisasterRecovery: () => {
    set({ disasterRecoveryStep: 1, disasterRecoveryResult: null, error: null });
  },

  cancelDisasterRecovery: () => {
    set({ disasterRecoveryStep: null, disasterRecoveryResult: null });
  },

  setDisasterRecoveryStep: (step) => {
    set({ disasterRecoveryStep: step });
  },

  disasterRecoveryExport: async (gitUrl, recoveryKey, exportPath) => {
    set({ loading: true, error: null });
    try {
      const result = await vaultService.disasterRecoveryExport(
        gitUrl,
        recoveryKey,
        exportPath,
      );
      set({
        disasterRecoveryStep: 3,
        disasterRecoveryResult: result,
        loading: false,
      });
      return result;
    } catch (e) {
      log.error("disasterRecoveryExport failed", e);
      set({ error: String(e), loading: false });
      return null;
    }
  },

  exportVault: async (format) => {
    set({ loading: true, error: null });
    try {
      const filePath = await vaultService.exportVault(format);
      set({ exportFilePath: filePath, loading: false });
      return filePath;
    } catch (e) {
      log.error("exportVault failed", e);
      set({ error: String(e), loading: false });
      return null;
    }
  },

  importVault: async (path, format) => {
    set({ loading: true, error: null });
    try {
      const count = await vaultService.importVault(path, format);
      await get().loadEntries();
      set({ loading: false });
      return count;
    } catch (e) {
      log.error("importVault failed", e);
      set({ error: String(e), loading: false });
      return null;
    }
  },
}));

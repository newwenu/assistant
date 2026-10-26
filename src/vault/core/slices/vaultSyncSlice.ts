import { createLogger } from "@/services/logger";
import { vaultService } from "../vault.service";
import type {
  SyncResult,
  SyncConfig,
  Readyness,
  ConflictResolution,
  SshKeyInfo,
} from "../vault.types";

const log = createLogger("vaultStore");

export interface VaultSyncSlice {
  syncReadyness: Readyness | null;
  syncResult: SyncResult | null;
  syncConfig: SyncConfig | null;
  syncing: boolean;
  sshAgentKeys: SshKeyInfo[];
  sshAgentRunning: boolean;

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
}

type FullState = VaultSyncSlice & {
  loading: boolean;
  error: string | null;
  loadEntries: () => Promise<void>;
};

export function createSyncSlice(
  set: (
    partial: Partial<FullState> | ((state: FullState) => Partial<FullState>),
  ) => void,
  get: () => FullState,
): VaultSyncSlice {
  return {
    syncReadyness: null,
    syncResult: null,
    syncConfig: null,
    syncing: false,
    sshAgentKeys: [],
    sshAgentRunning: false,

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
  };
}
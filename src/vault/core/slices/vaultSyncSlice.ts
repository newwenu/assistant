import { createLogger } from "@/services/logger";
import { vaultService } from "../vault.service";
import { vaultErrorCode, vaultErrorMessage } from "../vaultError";
import type {
  SyncResult,
  SyncConfig,
  Readyness,
  ConflictFile,
  ConflictResolution,
  PendingRemovals,
  SshKeyInfo,
  SshKeyMeta,
  VaultErrorCode,
} from "../vault.types";

const log = createLogger("vaultStore");

export interface VaultSyncSlice {
  syncReadyness: Readyness | null;
  syncResult: SyncResult | null;
  syncConfig: SyncConfig | null;
  syncConflicts: ConflictFile[];
  pendingRemovals: PendingRemovals | null;
  syncLastAt: number | null;
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
  loadSyncConflicts: () => Promise<void>;
  loadPendingRemovals: () => Promise<void>;
  loadSyncLastTime: () => Promise<void>;
  confirmRemovals: (ids: string[]) => Promise<void>;
  resolveConflict: (
    path: string,
    resolution: ConflictResolution,
    remoteCopySuffix?: string,
  ) => Promise<void>;
  gitInit: () => Promise<void>;
  gitClone: (url: string) => Promise<void>;
  gitAddRemote: (remote: string, url: string) => Promise<void>;
  gitSetRemote: (remote: string, url: string) => Promise<void>;
  gitRemoveRemote: (remote: string) => Promise<void>;
  gitTestConnection: (url?: string) => Promise<number | null>;
  syncAbort: () => Promise<void>;
  syncDiscard: () => Promise<void>;
  sshAddToAgent: (id: string, lifetime?: number) => Promise<string | null>;
  sshRemoveFromAgent: (fingerprint: string) => Promise<void>;
  sshListAgent: () => Promise<void>;
  sshAutoAddAll: () => Promise<string[]>;
  sshCheckAgentRunning: () => Promise<void>;
  sshParseKey: (privateKey: string) => Promise<SshKeyMeta | null>;
  sshKeyMeta: (id: string) => Promise<SshKeyMeta | null>;
}

type FullState = VaultSyncSlice & {
  loading: boolean;
  error: string | null;
  errorCode: VaultErrorCode | null;
  loadEntries: () => Promise<void>;
};

export function createSyncSlice(
  set: (
    partial: Partial<FullState> | ((state: FullState) => Partial<FullState>),
  ) => void,
  get: () => FullState,
): VaultSyncSlice {
  const failWith = (e: unknown, extra: Partial<FullState> = {}): void => {
    set({
      error: vaultErrorMessage(e),
      errorCode: vaultErrorCode(e),
      ...extra,
    });
  };

  const applySyncResult = (result: SyncResult) => {
    const pendingRemovals =
      typeof result === "object" &&
      result !== null &&
      "pendingRemovals" in result
        ? result.pendingRemovals
        : null;
    set({ syncResult: result, pendingRemovals });
  };

  return {
    syncReadyness: null,
    syncResult: null,
    syncConfig: null,
    syncConflicts: [],
    pendingRemovals: null,
    syncLastAt: null,
    syncing: false,
    sshAgentKeys: [],
    sshAgentRunning: false,

    syncPull: async () => {
      set({ syncing: true, error: null, errorCode: null });
      try {
        const result = await vaultService.syncPull();
        applySyncResult(result);
        set({ syncing: false });
        await get().loadEntries();
        await get().loadSyncReadyness();
        await get().loadSyncLastTime();
      } catch (e) {
        log.error("syncPull failed", e);
        failWith(e, { syncing: false });
      }
    },

    syncPush: async () => {
      set({ syncing: true, error: null, errorCode: null });
      try {
        const result = await vaultService.syncPush();
        applySyncResult(result);
        set({ syncing: false });
        await get().loadEntries();
        await get().loadSyncReadyness();
        await get().loadSyncLastTime();
      } catch (e) {
        log.error("syncPush failed", e);
        failWith(e, { syncing: false });
      }
    },

    syncNow: async () => {
      set({ syncing: true, error: null, errorCode: null });
      try {
        const result = await vaultService.syncNow();
        applySyncResult(result);
        set({ syncing: false });
        await get().loadEntries();
        await get().loadSyncReadyness();
        await get().loadSyncLastTime();
      } catch (e) {
        log.error("syncNow failed", e);
        failWith(e, { syncing: false });
      }
    },

    syncForcePush: async () => {
      set({ syncing: true, error: null, errorCode: null });
      try {
        const result = await vaultService.syncForcePush();
        applySyncResult(result);
        set({ syncing: false });
        await get().loadSyncLastTime();
      } catch (e) {
        log.error("syncForcePush failed", e);
        failWith(e, { syncing: false });
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
        const saved = await vaultService.getSyncConfig();
        set({ syncConfig: saved });
        await get().loadSyncReadyness();
      } catch (e) {
        log.error("saveSyncConfig failed", e);
        failWith(e);
      }
    },

    loadSyncReadyness: async () => {
      try {
        const readyness = await vaultService.syncReadyness();
        set({ syncReadyness: readyness });
        if (typeof readyness === "object" && readyness !== null) {
          await get().loadSyncConflicts();
        } else {
          set({ syncConflicts: [] });
        }
      } catch (e) {
        log.error("loadSyncReadyness failed", e);
      }
    },

    loadSyncConflicts: async () => {
      try {
        const syncConflicts = await vaultService.syncConflicts();
        set({ syncConflicts });
      } catch (e) {
        log.error("loadSyncConflicts failed", e);
        set({ syncConflicts: [] });
      }
    },

    loadPendingRemovals: async () => {
      try {
        const pendingRemovals = await vaultService.syncPendingRemovals();
        set({ pendingRemovals });
      } catch (e) {
        log.error("loadPendingRemovals failed", e);
      }
    },

    loadSyncLastTime: async () => {
      try {
        const syncLastAt = await vaultService.syncLastTime();
        set({ syncLastAt });
      } catch (e) {
        log.error("loadSyncLastTime failed", e);
      }
    },

    confirmRemovals: async (ids) => {
      set({ syncing: true, error: null, errorCode: null });
      try {
        await vaultService.syncConfirmRemovals(ids);
        set({ pendingRemovals: null, syncResult: "success" });
        await get().loadEntries();
        await get().loadSyncReadyness();
        await get().loadSyncLastTime();
        set({ syncing: false });
      } catch (e) {
        log.error("confirmRemovals failed", e);
        failWith(e, { syncing: false });
      }
    },

    resolveConflict: async (path, resolution, remoteCopySuffix) => {
      try {
        await vaultService.resolveConflict(path, resolution, remoteCopySuffix);
        await get().loadSyncReadyness();
        await get().loadSyncConflicts();
        await get().loadEntries();
        await get().loadSyncLastTime();
      } catch (e) {
        log.error("resolveConflict failed", e);
        failWith(e);
      }
    },

    gitInit: async () => {
      set({ loading: true, error: null, errorCode: null });
      try {
        await vaultService.gitInit();
        await get().loadSyncReadyness();
        await get().loadSyncConfig();
        set({ loading: false });
      } catch (e) {
        log.error("gitInit failed", e);
        failWith(e, { loading: false });
      }
    },

    gitClone: async (url) => {
      set({ loading: true, error: null, errorCode: null });
      try {
        await vaultService.gitClone(url);
        await get().loadSyncReadyness();
        await get().loadSyncConfig();
        set({ loading: false });
      } catch (e) {
        log.error("gitClone failed", e);
        failWith(e, { loading: false });
      }
    },

    gitAddRemote: async (remote, url) => {
      try {
        await vaultService.gitAddRemote(remote, url);
        await get().loadSyncConfig();
      } catch (e) {
        log.error("gitAddRemote failed", e);
        failWith(e);
      }
    },

    gitSetRemote: async (remote, url) => {
      try {
        await vaultService.gitSetRemote(remote, url);
        await get().loadSyncConfig();
      } catch (e) {
        log.error("gitSetRemote failed", e);
        failWith(e);
      }
    },

    gitRemoveRemote: async (remote) => {
      try {
        await vaultService.gitRemoveRemote(remote);
        await get().loadSyncConfig();
        await get().loadSyncReadyness();
      } catch (e) {
        log.error("gitRemoveRemote failed", e);
        failWith(e);
      }
    },

    gitTestConnection: async (url) => {
      try {
        const count = await vaultService.gitTestConnection(url);
        return count;
      } catch (e) {
        log.error("gitTestConnection failed", e);
        failWith(e);
        return null;
      }
    },

    syncAbort: async () => {
      set({ syncing: true, error: null, errorCode: null });
      try {
        await vaultService.syncAbort();
        await get().loadSyncReadyness();
        set({ syncing: false });
      } catch (e) {
        log.error("syncAbort failed", e);
        failWith(e, { syncing: false });
      }
    },

    syncDiscard: async () => {
      set({ syncing: true, error: null, errorCode: null });
      try {
        await vaultService.syncDiscard();
        await get().loadSyncReadyness();
        set({ syncing: false });
      } catch (e) {
        log.error("syncDiscard failed", e);
        failWith(e, { syncing: false });
      }
    },

    sshAddToAgent: async (id, lifetime) => {
      try {
        const fingerprint = await vaultService.sshAddToAgent(id, lifetime);
        await get().sshListAgent();
        return fingerprint;
      } catch (e) {
        log.error("sshAddToAgent failed", e);
        failWith(e);
        return null;
      }
    },

    sshRemoveFromAgent: async (fingerprint) => {
      try {
        await vaultService.sshRemoveFromAgent(fingerprint);
        await get().sshListAgent();
      } catch (e) {
        log.error("sshRemoveFromAgent failed", e);
        failWith(e);
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
        failWith(e);
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

    sshParseKey: async (privateKey) => {
      try {
        return await vaultService.sshParseKey(privateKey);
      } catch (e) {
        log.error("sshParseKey failed", e);
        return null;
      }
    },

    sshKeyMeta: async (id) => {
      try {
        return await vaultService.sshKeyMeta(id);
      } catch (e) {
        log.error("sshKeyMeta failed", e);
        return null;
      }
    },
  };
}

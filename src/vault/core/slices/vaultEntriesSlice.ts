import { createLogger } from "@/services/logger";
import { vaultService } from "../vault.service";
import { vaultErrorCode, vaultErrorMessage } from "../vaultError";
import type {
  VaultEntrySummary,
  VaultEntryDetail,
  VaultEntryCreate,
  VaultEntryUpdate,
  VaultCategory,
  VaultErrorCode,
} from "../vault.types";

const log = createLogger("vaultStore");

export interface VaultEntriesSlice {
  entries: VaultEntrySummary[];
  selectedEntry: VaultEntryDetail | null;
  searchQuery: string;
  categoryFilter: VaultCategory | null;
  tagFilter: string | null;
  deletedEntries: VaultEntrySummary[];

  loadEntries: (options?: { silent?: boolean }) => Promise<void>;
  selectEntry: (id: string) => Promise<void>;
  clearSelectedEntry: () => void;
  createEntry: (entry: VaultEntryCreate) => Promise<VaultEntryDetail>;
  updateEntry: (
    id: string,
    entry: VaultEntryUpdate,
  ) => Promise<VaultEntryDetail>;
  setFavorite: (id: string, isFavorite: boolean) => Promise<VaultEntryDetail>;
  deleteEntry: (id: string) => Promise<void>;
  loadDeletedEntries: () => Promise<void>;
  restoreEntry: (id: string) => Promise<void>;
  purgeDeleted: () => Promise<number>;
  setSearchQuery: (query: string) => void;
  setCategoryFilter: (category: VaultCategory | null) => void;
  setTagFilter: (tag: string | null) => void;
}

type FullState = VaultEntriesSlice & {
  loading: boolean;
  error: string | null;
  errorCode: VaultErrorCode | null;
};

export function createEntriesSlice(
  set: (
    partial: Partial<FullState> | ((state: FullState) => Partial<FullState>),
  ) => void,
  get: () => FullState,
): VaultEntriesSlice {
  let selectSeq = 0;

  return {
    entries: [],
    selectedEntry: null,
    searchQuery: "",
    categoryFilter: null,
    tagFilter: null,
    deletedEntries: [],

    loadEntries: async (options) => {
      const silent = options?.silent ?? false;
      if (!silent) set({ loading: true, error: null, errorCode: null });
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
        set({
          error: vaultErrorMessage(e),
          errorCode: vaultErrorCode(e),
          loading: false,
        });
      }
    },

    selectEntry: async (id) => {
      const seq = ++selectSeq;
      try {
        const detail = await vaultService.getEntry(id);
        if (seq !== selectSeq) return;
        set({ selectedEntry: detail });
      } catch (e) {
        if (seq !== selectSeq) return;
        log.error("selectEntry failed", e);
        set({ error: vaultErrorMessage(e), errorCode: vaultErrorCode(e) });
      }
    },

    clearSelectedEntry: () => set({ selectedEntry: null }),

    createEntry: async (entry) => {
      set({ error: null, errorCode: null });
      try {
        const detail = await vaultService.createEntry(entry);
        await get().loadEntries({ silent: true });
        return detail;
      } catch (e) {
        log.error("createEntry failed", e);
        set({ error: vaultErrorMessage(e), errorCode: vaultErrorCode(e) });
        throw e;
      }
    },

    updateEntry: async (id, entry) => {
      set({ error: null, errorCode: null });
      try {
        const detail = await vaultService.updateEntry(id, entry);
        if (get().selectedEntry?.id === id) set({ selectedEntry: detail });
        await get().loadEntries({ silent: true });
        return detail;
      } catch (e) {
        log.error("updateEntry failed", e);
        set({ error: vaultErrorMessage(e), errorCode: vaultErrorCode(e) });
        throw e;
      }
    },

    setFavorite: async (id, isFavorite) => {
      try {
        const detail = await vaultService.setFavorite(id, isFavorite);
        if (get().selectedEntry?.id === id) set({ selectedEntry: detail });
        await get().loadEntries({ silent: true });
        return detail;
      } catch (e) {
        log.error("setFavorite failed", e);
        set({ error: vaultErrorMessage(e), errorCode: vaultErrorCode(e) });
        throw e;
      }
    },

    deleteEntry: async (id) => {
      set({ error: null, errorCode: null });
      try {
        await vaultService.deleteEntry(id);
        const { selectedEntry, entries } = get();
        set({
          entries: entries.filter((entry) => entry.id !== id),
          selectedEntry: selectedEntry?.id === id ? null : selectedEntry,
        });
        await get().loadEntries({ silent: true });
      } catch (e) {
        log.error("deleteEntry failed", e);
        set({ error: vaultErrorMessage(e), errorCode: vaultErrorCode(e) });
      }
    },

    loadDeletedEntries: async () => {
      try {
        const deletedEntries = await vaultService.listDeletedEntries();
        set({ deletedEntries });
      } catch (e) {
        log.error("loadDeletedEntries failed", e);
        set({ error: vaultErrorMessage(e), errorCode: vaultErrorCode(e) });
      }
    },

    restoreEntry: async (id) => {
      try {
        await vaultService.restoreEntry(id);
        await Promise.all([
          get().loadEntries({ silent: true }),
          get().loadDeletedEntries(),
        ]);
      } catch (e) {
        log.error("restoreEntry failed", e);
        set({ error: vaultErrorMessage(e), errorCode: vaultErrorCode(e) });
      }
    },

    purgeDeleted: async () => {
      try {
        const purged = await vaultService.purgeDeleted();
        await get().loadDeletedEntries();
        return purged;
      } catch (e) {
        log.error("purgeDeleted failed", e);
        set({ error: vaultErrorMessage(e), errorCode: vaultErrorCode(e) });
        return 0;
      }
    },

    setSearchQuery: (query) => {
      set({ searchQuery: query });
    },

    setCategoryFilter: (category) => {
      set({ categoryFilter: category });
    },

    setTagFilter: (tag) => {
      set({ tagFilter: tag });
    },
  };
}

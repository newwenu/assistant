import { createLogger } from "@/services/logger";
import { vaultService } from "../vault.service";
import type {
  VaultEntrySummary,
  VaultEntryDetail,
  VaultEntryCreate,
  VaultEntryUpdate,
  VaultCategory,
} from "../vault.types";

const log = createLogger("vaultStore");

export interface VaultEntriesSlice {
  entries: VaultEntrySummary[];
  selectedEntry: VaultEntryDetail | null;
  searchQuery: string;
  categoryFilter: VaultCategory | null;
  tagFilter: string | null;

  loadEntries: () => Promise<void>;
  selectEntry: (id: string) => Promise<void>;
  clearSelectedEntry: () => void;
  createEntry: (entry: VaultEntryCreate) => Promise<string>;
  updateEntry: (id: string, entry: VaultEntryUpdate) => Promise<void>;
  deleteEntry: (id: string) => Promise<void>;
  setSearchQuery: (query: string) => void;
  setCategoryFilter: (category: VaultCategory | null) => void;
  setTagFilter: (tag: string | null) => void;
}

type FullState = VaultEntriesSlice & {
  loading: boolean;
  error: string | null;
};

export function createEntriesSlice(
  set: (
    partial: Partial<FullState> | ((state: FullState) => Partial<FullState>),
  ) => void,
  get: () => FullState,
): VaultEntriesSlice {
  return {
    entries: [],
    selectedEntry: null,
    searchQuery: "",
    categoryFilter: null,
    tagFilter: null,

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
        const { selectedEntry } = get();
        if (selectedEntry?.id === id) {
          await get().selectEntry(id);
        }
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

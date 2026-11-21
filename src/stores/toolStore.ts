import { create } from "zustand";
import { toolService } from "../services/tool/toolService";
import Fuse from "fuse.js";
import i18n, { translateKey } from "@/i18n";
import { tools, toolsByCategory } from "../tools/framework/registry";
import type {
  ToolDefinition,
  ToolCategoryGroup,
} from "../tools/framework/types";
import { TOOL_SEARCH_THRESHOLD, RECENT_TOOLS_LIMIT } from "../config/constants";
import { createLogger } from "@/services/system/logger";

const log = createLogger("toolStore");

interface ToolState {
  tools: ToolDefinition[];
  toolsByCategory: ToolCategoryGroup[];
  favoriteToolIds: string[];
  recentTools: string[];
  searchQuery: string;
  searchResults: ToolDefinition[];
  fuse: Fuse<ToolDefinition>;

  loadFavorites: () => Promise<void>;
  toggleFavorite: (toolId: string) => Promise<void>;
  reorderFavorites: (toolIds: string[]) => Promise<void>;
  loadRecentTools: (limit?: number) => Promise<void>;
  recordUsage: (toolId: string) => Promise<void>;
  search: (query: string) => void;
}

function normalizeFavoriteIds(ids: unknown): string[] {
  if (!Array.isArray(ids)) return [];
  return [...new Set(ids.filter((id): id is string => typeof id === "string"))];
}

function buildFuseInstance(): Fuse<ToolDefinition> {
  return new Fuse(
    tools.map((t) => ({
      ...t,
      name: translateKey(t.name),
      description: translateKey(t.description),
    })),
    {
      keys: [
        { name: "name", weight: 2 },
        { name: "keywords", weight: 1.5 },
        { name: "description", weight: 1 },
      ],
      threshold: TOOL_SEARCH_THRESHOLD,
    },
  );
}

let fuseInstance = buildFuseInstance();

export const useToolStore = create<ToolState>((set) => ({
  tools,
  toolsByCategory,
  favoriteToolIds: [],
  recentTools: [],
  searchQuery: "",
  searchResults: [],
  fuse: fuseInstance,

  loadFavorites: async () => {
    try {
      const ids = await toolService.getFavoriteTools();
      set({ favoriteToolIds: normalizeFavoriteIds(ids) });
    } catch (e) {
      log.warn("loadFavorites failed, trying localStorage fallback", e);
      const stored = localStorage.getItem("favoriteToolIds");
      if (stored) {
        try {
          set({ favoriteToolIds: normalizeFavoriteIds(JSON.parse(stored)) });
        } catch {
          /* corrupted localStorage, ignore */
        }
      }
    }
  },

  toggleFavorite: async (toolId: string) => {
    try {
      const isFav = await toolService.toggleFavorite(toolId);
      set((s) => ({
        favoriteToolIds: isFav
          ? s.favoriteToolIds.includes(toolId)
            ? s.favoriteToolIds
            : [...s.favoriteToolIds, toolId]
          : s.favoriteToolIds.filter((id) => id !== toolId),
      }));
    } catch (e) {
      log.warn("toggleFavorite failed, using localStorage fallback", e);
      set((s) => {
        const isFav = s.favoriteToolIds.includes(toolId);
        const next = isFav
          ? s.favoriteToolIds.filter((id) => id !== toolId)
          : [...s.favoriteToolIds, toolId];
        localStorage.setItem("favoriteToolIds", JSON.stringify(next));
        return { favoriteToolIds: next };
      });
    }
  },

  reorderFavorites: async (toolIds: string[]) => {
    const ids = normalizeFavoriteIds(toolIds);
    set({ favoriteToolIds: ids });
    try {
      await toolService.reorderFavorites(ids);
    } catch (e) {
      log.warn("reorderFavorites failed, saving to localStorage", e);
      localStorage.setItem("favoriteToolIds", JSON.stringify(ids));
    }
  },

  loadRecentTools: async (limit?: number) => {
    try {
      const ids = await toolService.getRecentTools(limit ?? RECENT_TOOLS_LIMIT);
      set({ recentTools: ids });
    } catch (e) {
      log.warn("loadRecentTools failed", e);
    }
  },

  recordUsage: async (toolId: string) => {
    set((s) => ({
      recentTools: [toolId, ...s.recentTools.filter((id) => id !== toolId)],
    }));
    try {
      await toolService.recordUsage(toolId);
    } catch (e) {
      log.warn("recordUsage failed", { toolId, error: e });
    }
  },

  search: (query: string) => {
    if (!query.trim()) {
      set({ searchQuery: query, searchResults: [] });
      return;
    }
    const results = fuseInstance.search(query).map((r) => r.item);
    set({ searchQuery: query, searchResults: results });
  },
}));

i18n.on("languageChanged", () => {
  fuseInstance = buildFuseInstance();
  useToolStore.setState({ fuse: fuseInstance });
});

import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import Fuse from "fuse.js";
import { tools, toolsByCategory } from "../tools/registry";
import type { ToolDefinition, ToolCategoryGroup } from "../tools/types";
import { TOOL_SEARCH_THRESHOLD, RECENT_TOOLS_LIMIT } from "../config/constants";

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

const fuseInstance = new Fuse(tools, {
  keys: [
    { name: "name", weight: 2 },
    { name: "keywords", weight: 1.5 },
    { name: "description", weight: 1 },
  ],
  threshold: TOOL_SEARCH_THRESHOLD,
});

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
      const ids = (await invoke("get_favorite_tools")) as string[];
      set({ favoriteToolIds: ids });
    } catch {
      const stored = localStorage.getItem("favoriteToolIds");
      if (stored) {
        set({ favoriteToolIds: JSON.parse(stored) });
      }
    }
  },

  toggleFavorite: async (toolId: string) => {
    try {
      const isFav = await invoke<boolean>("toggle_tool_favorite", { toolId });
      set((s) => ({
        favoriteToolIds: isFav
          ? [...s.favoriteToolIds, toolId]
          : s.favoriteToolIds.filter((id) => id !== toolId),
      }));
    } catch {
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
    set({ favoriteToolIds: toolIds });
    try {
      await invoke("set_favorite_tools_order", { toolIds });
    } catch {
      localStorage.setItem("favoriteToolIds", JSON.stringify(toolIds));
    }
  },

  loadRecentTools: async (limit?: number) => {
    try {
      const ids = (await invoke("get_recent_tools", {
        limit: limit ?? RECENT_TOOLS_LIMIT,
      })) as string[];
      set({ recentTools: ids });
    } catch {
      // fallback: recentTools already updated optimistically
    }
  },

  recordUsage: async (toolId: string) => {
    set((s) => ({
      recentTools: [toolId, ...s.recentTools.filter((id) => id !== toolId)],
    }));
    try {
      await invoke("record_tool_usage", { toolId });
    } catch {
      // fallback: usage recorded optimistically in state
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

import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import Fuse from "fuse.js";
import i18n, { translateKey } from "@/i18n";
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

i18n.on("languageChanged", () => {
  fuseInstance = buildFuseInstance();
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
      const ids = await invoke<string[]>("get_favorite_tools");
      set({ favoriteToolIds: ids });
    } catch {
      const stored = localStorage.getItem("favoriteToolIds");
      if (stored) {
        set({ favoriteToolIds: JSON.parse(stored) as string[] });
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
      const ids = await invoke<string[]>("get_recent_tools", {
        limit: limit ?? RECENT_TOOLS_LIMIT,
      });
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

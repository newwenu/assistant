import { tauriInvoke as invoke } from "../system/tauriInvoke";

export const toolService = {
  getFavoriteTools: (): Promise<string[]> =>
    invoke<string[]>("get_favorite_tools").then((r) => r || []),

  toggleFavorite: (toolId: string): Promise<boolean> =>
    invoke<boolean>("toggle_tool_favorite", { toolId }),

  reorderFavorites: (toolIds: string[]): Promise<void> =>
    invoke("set_favorite_tools_order", { toolIds }),

  getRecentTools: (limit?: number): Promise<string[]> =>
    invoke<string[]>("get_recent_tools", { limit }).then((r) => r || []),

  recordUsage: (toolId: string): Promise<void> =>
    invoke("record_tool_usage", { toolId }),
};

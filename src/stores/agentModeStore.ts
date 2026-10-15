import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
export type { AgentMode, AgentModeInput } from "@/settings/agentMode/types";
export { toModeInput } from "@/settings/agentMode/types";
import type { AgentMode } from "@/settings/agentMode/types";
import { toModeInput } from "@/settings/agentMode/types";

interface AgentModeState {
  modes: AgentMode[];
  selectedModeId: string | null;
  loading: boolean;
  loadModes: () => Promise<void>;
  selectMode: (modeId: string | null) => void;
  saveMode: (mode: Partial<AgentMode> & { id?: string }) => Promise<void>;
  deleteMode: (id: string) => Promise<void>;
  resetBuiltinMode: (id: string) => Promise<void>;
}

export const useAgentModeStore = create<AgentModeState>((set, get) => ({
  modes: [],
  selectedModeId: "builtin-default",
  loading: false,

  loadModes: async () => {
    set({ loading: true });
    try {
      const result = await invoke("list_agent_modes");
      const modes = (result as AgentMode[]) || [];
      set({ modes, loading: false });
      const state = get();
      if (
        state.selectedModeId &&
        !modes.some((m) => m.id === state.selectedModeId)
      ) {
        set({ selectedModeId: "builtin-default" });
      }
    } catch (e) {
      console.error("Failed to load agent modes:", e);
      set({ loading: false });
    }
  },

  selectMode: (modeId: string | null) => {
    set({ selectedModeId: modeId });
  },

  saveMode: async (mode: Partial<AgentMode> & { id?: string }) => {
    await invoke("save_agent_mode", { mode: toModeInput(mode) });
    await get().loadModes();
  },

  deleteMode: async (id: string) => {
    await invoke("delete_agent_mode", { id });
    const state = get();
    if (state.selectedModeId === id) {
      set({ selectedModeId: "builtin-default" });
    }
    await get().loadModes();
  },

  resetBuiltinMode: async (id: string) => {
    await invoke("reset_builtin_mode", { id });
    await get().loadModes();
  },
}));

import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";

export interface AgentMode {
  id: string;
  name: string;
  description: string;
  kind: "default" | "quick" | "deep" | "custom";
  prompt: string;
  model: string | null;
  temperature: number | null;
  max_tokens: number | null;
  reasoning_enabled: boolean | null;
  reasoning_effort: string | null;
  tools: string[];
  presentation_rank: number;
  is_builtin: boolean;
  created_at: number;
  updated_at: number;
}

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
    const payload: Record<string, unknown> = {
      id: mode.id || "",
      name: mode.name || "",
      description: mode.description || "",
      kind: mode.kind || "custom",
      prompt: mode.prompt || "",
      model: mode.model ?? null,
      temperature: mode.temperature ?? null,
      max_tokens: mode.max_tokens ?? null,
      reasoning_enabled: mode.reasoning_enabled ?? null,
      reasoning_effort: mode.reasoning_effort ?? null,
      tools: mode.tools || [],
      presentation_rank: mode.presentation_rank ?? 99,
      is_builtin: mode.is_builtin ?? false,
      created_at: mode.created_at ?? Date.now(),
    };
    await invoke("save_agent_mode", { mode: payload });
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
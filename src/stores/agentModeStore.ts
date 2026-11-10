import { create } from "zustand";
import { agentModeService } from "../services/agentModeService";
import { createLogger } from "@/services/logger";
import type {
  AgentModeDto,
  AgentModeInputDto,
} from "@/types/protocol.contract";
import { DEFAULT_TOOL_CONFIG } from "@/chat/core/toolConfig";

export type AgentMode = AgentModeDto;
export type AgentModeInput = AgentModeInputDto;

export function toModeInput(
  mode: Partial<AgentMode> & { id?: string },
): AgentModeInput {
  return {
    id: mode.id || "",
    name: mode.name || "",
    description: mode.description || "",
    kind: mode.kind || "chat",
    prompt: mode.prompt || "",
    model: mode.model ?? null,
    temperature: mode.temperature ?? null,
    maxTokens: mode.maxTokens ?? null,
    reasoningEnabled: mode.reasoningEnabled ?? null,
    reasoningEffort: mode.reasoningEffort ?? null,
    presentationRank: mode.presentationRank ?? 99,
    isBuiltin: mode.isBuiltin ?? false,
    createdAt: mode.createdAt ?? Date.now(),
    toolConfig: mode.toolConfig ?? DEFAULT_TOOL_CONFIG,
    // 原样透传三态（I43 §6）：`undefined` → 缺省、`null` → 缺省、`""` → 清除，
    // 因此禁止 `?? null` 兜底，否则 `""` 会被折叠成 `null`，头像永远清不掉。
    avatar: mode.avatar,
  };
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

const log = createLogger("agentModeStore");

export const useAgentModeStore = create<AgentModeState>((set, get) => ({
  modes: [],
  selectedModeId: "builtin-default",
  loading: false,

  loadModes: async () => {
    set({ loading: true });
    try {
      const modes = await agentModeService.listModes();
      set({ modes: modes || [], loading: false });
      const state = get();
      if (
        state.selectedModeId &&
        !modes.some((m) => m.id === state.selectedModeId)
      ) {
        set({ selectedModeId: "builtin-default" });
      }
    } catch (e) {
      log.error("Failed to load agent modes", e);
      set({ loading: false });
    }
  },

  selectMode: (modeId: string | null) => {
    set({ selectedModeId: modeId });
  },

  saveMode: async (mode: Partial<AgentMode> & { id?: string }) => {
    await agentModeService.saveMode(toModeInput(mode));
    await get().loadModes();
  },

  deleteMode: async (id: string) => {
    await agentModeService.deleteMode(id);
    const state = get();
    if (state.selectedModeId === id) {
      set({ selectedModeId: "builtin-default" });
    }
    await get().loadModes();
  },

  resetBuiltinMode: async (id: string) => {
    await agentModeService.resetBuiltinMode(id);
    await get().loadModes();
  },
}));

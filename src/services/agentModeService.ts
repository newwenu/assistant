import { invoke } from "@tauri-apps/api/core";
import type { AgentMode, AgentModeInput } from "../settings/agentMode/types";

export const agentModeService = {
  listModes: (): Promise<AgentMode[]> =>
    invoke<AgentMode[]>("list_agent_modes").then((r) => r || []),

  saveMode: (mode: AgentModeInput): Promise<void> =>
    invoke("save_agent_mode", { mode }),

  deleteMode: (id: string): Promise<void> =>
    invoke("delete_agent_mode", { id }),

  resetBuiltinMode: (id: string): Promise<void> =>
    invoke("reset_builtin_mode", { id }),
};

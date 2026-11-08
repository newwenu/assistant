import { tauriInvoke as invoke } from "./tauriInvoke";
import type {
  AgentModeDto,
  AgentModeInputDto,
} from "../types/protocol.contract";

export const agentModeService = {
  listModes: (): Promise<AgentModeDto[]> =>
    invoke<AgentModeDto[]>("list_agent_modes").then((r) => r || []),

  saveMode: (mode: AgentModeInputDto): Promise<void> =>
    invoke("save_agent_mode", { mode }),

  deleteMode: (id: string): Promise<void> =>
    invoke("delete_agent_mode", { id }),

  resetBuiltinMode: (id: string): Promise<void> =>
    invoke("reset_builtin_mode", { id }),
};

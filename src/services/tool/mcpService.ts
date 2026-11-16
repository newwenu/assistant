import type {
  McpServerCreateDto,
  McpServerDto,
  McpServerUpdateDto,
} from "@/types/protocol.contract";
import { tauriInvoke as invoke } from "../system/tauriInvoke";

export const mcpService = {
  listServers: (): Promise<McpServerDto[]> =>
    invoke<McpServerDto[]>("list_mcp_servers").then((r) => r || []),

  createServer: (input: McpServerCreateDto): Promise<McpServerDto> =>
    invoke<McpServerDto>("create_mcp_server", { input }),

  updateServer: (input: McpServerUpdateDto): Promise<McpServerDto> =>
    invoke<McpServerDto>("update_mcp_server", { input }),

  deleteServer: (id: string): Promise<void> =>
    invoke<void>("delete_mcp_server", { id }),

  setEnabled: (id: string, enabled: boolean): Promise<McpServerDto> =>
    invoke<McpServerDto>("set_mcp_server_enabled", { id, enabled }),
};

import { create } from "zustand";
import { mcpService } from "@/services/tool/mcpService";
import { createLogger } from "@/services/system/logger";
import type {
  McpServerCreateDto,
  McpServerDto,
  McpServerUpdateDto,
} from "@/types/protocol.contract";

interface McpServerState {
  servers: McpServerDto[];
  loading: boolean;

  loadServers: () => Promise<void>;
  createServer: (input: McpServerCreateDto) => Promise<McpServerDto>;
  updateServer: (input: McpServerUpdateDto) => Promise<McpServerDto>;
  deleteServer: (id: string) => Promise<void>;
  setEnabled: (id: string, enabled: boolean) => Promise<void>;
}

const log = createLogger("mcpServerStore");

function replaceServer(
  servers: McpServerDto[],
  next: McpServerDto,
): McpServerDto[] {
  return servers.map((server) => (server.id === next.id ? next : server));
}

export const useMcpServerStore = create<McpServerState>((set) => ({
  servers: [],
  loading: false,

  loadServers: async () => {
    set({ loading: true });
    try {
      const servers = await mcpService.listServers();
      set({ servers, loading: false });
    } catch (e) {
      log.error("Failed to load mcp servers", e);
      set({ loading: false });
    }
  },

  createServer: async (input: McpServerCreateDto) => {
    const created = await mcpService.createServer(input);
    set((state) => ({ servers: [...state.servers, created] }));
    return created;
  },

  updateServer: async (input: McpServerUpdateDto) => {
    const updated = await mcpService.updateServer(input);
    set((state) => ({ servers: replaceServer(state.servers, updated) }));
    return updated;
  },

  deleteServer: async (id: string) => {
    await mcpService.deleteServer(id);
    set((state) => ({
      servers: state.servers.filter((server) => server.id !== id),
    }));
  },

  setEnabled: async (id: string, enabled: boolean) => {
    const updated = await mcpService.setEnabled(id, enabled);
    set((state) => ({ servers: replaceServer(state.servers, updated) }));
  },
}));

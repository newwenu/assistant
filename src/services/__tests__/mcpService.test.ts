import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../tauriInvoke", () => ({
  tauriInvoke: vi.fn(),
}));

import { tauriInvoke as invoke } from "../tauriInvoke";
import { mcpService } from "../mcpService";
import type { McpServerDto } from "@/types/protocol.contract";

const mockInvoke = vi.mocked(invoke);

const SERVER: McpServerDto = {
  id: "filesystem",
  name: "filesystem",
  transport: "stdio",
  enabled: false,
  status: "stopped",
  lastError: null,
  command: "npx",
  args: [],
  cwd: null,
  envKeys: [],
  callTimeoutMs: null,
  configError: null,
  createdAt: 0,
  updatedAt: 0,
};

describe("mcpService", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("lists servers and tolerates a null payload", async () => {
    mockInvoke.mockResolvedValue(null);
    await expect(mcpService.listServers()).resolves.toEqual([]);
    expect(mockInvoke).toHaveBeenCalledWith("list_mcp_servers");
  });

  it("creates a server with the create DTO as `input`", async () => {
    mockInvoke.mockResolvedValue(SERVER);
    const input = {
      name: "filesystem",
      launch: { command: "npx", args: [], cwd: null, env: {} },
    };
    await mcpService.createServer(input);
    expect(mockInvoke).toHaveBeenCalledWith("create_mcp_server", { input });
  });

  it("updates a server with the update DTO as `input`", async () => {
    mockInvoke.mockResolvedValue(SERVER);
    await mcpService.updateServer({ id: "filesystem", name: "renamed" });
    expect(mockInvoke).toHaveBeenCalledWith("update_mcp_server", {
      input: { id: "filesystem", name: "renamed" },
    });
  });

  it("deletes by id", async () => {
    mockInvoke.mockResolvedValue(undefined);
    await mcpService.deleteServer("filesystem");
    expect(mockInvoke).toHaveBeenCalledWith("delete_mcp_server", {
      id: "filesystem",
    });
  });

  it("toggles enabled by id", async () => {
    mockInvoke.mockResolvedValue({ ...SERVER, enabled: true });
    await mcpService.setEnabled("filesystem", true);
    expect(mockInvoke).toHaveBeenCalledWith("set_mcp_server_enabled", {
      id: "filesystem",
      enabled: true,
    });
  });
});

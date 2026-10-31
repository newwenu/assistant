import { beforeEach, describe, expect, it, vi } from "vitest";

const calls: { command: string; args: unknown }[] = [];

vi.mock("@/services/tauriInvoke", () => ({
  tauriInvoke: (command: string, args?: unknown): Promise<unknown> => {
    calls.push({ command, args });
    return Promise.resolve(undefined);
  },
}));

import { vaultService } from "../vault.service";

function lastCall() {
  return calls[calls.length - 1];
}

describe("vaultService SSH 全链路走插件命名空间", () => {
  beforeEach(() => {
    calls.length = 0;
  });

  it("七个 SSH 命令都带 plugin:vault| 前缀", async () => {
    await vaultService.sshAddToAgent("entry-1", 3600);
    await vaultService.sshRemoveFromAgent("SHA256:abc");
    await vaultService.sshListAgent();
    await vaultService.sshAutoAddAll();
    await vaultService.sshIsAgentRunning();
    await vaultService.sshParseKey("PRIVATE KEY");
    await vaultService.sshKeyMeta("entry-1");

    const commands = calls.map((call) => call.command);
    expect(commands).toEqual([
      "plugin:vault|vault_ssh_add_to_agent",
      "plugin:vault|vault_ssh_remove_from_agent",
      "plugin:vault|vault_ssh_list_agent",
      "plugin:vault|vault_ssh_auto_add_all",
      "plugin:vault|vault_ssh_is_agent_running",
      "plugin:vault|vault_ssh_parse_key",
      "plugin:vault|vault_ssh_key_meta",
    ]);
  });

  it("SSH 参数按后端 camelCase 契约透传", async () => {
    await vaultService.sshAddToAgent("entry-1", 3600);
    expect(lastCall().args).toEqual({ id: "entry-1", lifetime: 3600 });

    await vaultService.sshRemoveFromAgent("SHA256:abc");
    expect(lastCall().args).toEqual({ fingerprint: "SHA256:abc" });

    await vaultService.sshParseKey("PRIVATE KEY");
    expect(lastCall().args).toEqual({ privateKey: "PRIVATE KEY" });

    await vaultService.sshKeyMeta("entry-1");
    expect(lastCall().args).toEqual({ id: "entry-1" });
  });

  it("无参 SSH 命令不传 args", async () => {
    await vaultService.sshListAgent();
    expect(lastCall().args).toBeUndefined();

    await vaultService.sshAutoAddAll();
    expect(lastCall().args).toBeUndefined();

    await vaultService.sshIsAgentRunning();
    expect(lastCall().args).toBeUndefined();
  });
});

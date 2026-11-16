import { beforeEach, describe, expect, it, vi } from "vitest";

const calls: { command: string; args: unknown }[] = [];

vi.mock("@/services/system/tauriInvoke", () => ({
  tauriInvoke: (command: string, args?: unknown): Promise<unknown> => {
    calls.push({ command, args });
    if (command === "plugin:vault|vault_conflict_reveal") {
      return Promise.resolve({ value: "s3cret", expiresAt: 1_700_000_000_000 });
    }
    return Promise.resolve({ path: "/tmp/vault_export.json", count: 3 });
  },
}));

import { vaultService } from "../vault.service";

describe("vault 明文导出必须携带确认令牌", () => {
  beforeEach(() => {
    calls.length = 0;
  });

  it("导出命令透传 confirmToken 与路径", async () => {
    const result = await vaultService.exportVaultToFile(
      "json",
      "/tmp/vault_export.json",
      "EXPORT-PLAINTEXT",
    );

    expect(result).toEqual({ path: "/tmp/vault_export.json", count: 3 });

    const call = calls[0];
    expect(call.command).toBe("plugin:vault|vault_export_to_file");
    expect(call.args).toEqual({
      format: "json",
      path: "/tmp/vault_export.json",
      confirmToken: "EXPORT-PLAINTEXT",
    });
  });

  it("冲突揭示按 RevealResult 读取明文与过期时间", async () => {
    const result = await vaultService.revealConflictField(
      "entries/e1.enc",
      "local",
      "password",
    );

    expect(calls[0].command).toBe("plugin:vault|vault_conflict_reveal");
    expect(calls[0].args).toEqual({
      path: "entries/e1.enc",
      side: "local",
      field: "password",
    });
    expect(result).toEqual({ value: "s3cret", expiresAt: 1_700_000_000_000 });
  });
});

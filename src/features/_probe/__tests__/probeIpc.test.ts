import { beforeEach, describe, expect, it, vi } from "vitest";

const calls: { command: string; args: unknown }[] = [];

vi.mock("@/services/tauriInvoke", () => ({
  tauriInvoke: (command: string, args?: unknown): Promise<unknown> => {
    calls.push({ command, args });
    return Promise.resolve(undefined);
  },
}));

import { probeEcho } from "../ipc";
import { probeConfSlot } from "../conf";

describe("探针全链路走插件命名空间", () => {
  beforeEach(() => {
    calls.length = 0;
  });

  it("probe_echo 命中 plugin:probe| 前缀并透传参数", async () => {
    await probeEcho("hello");
    expect(calls).toHaveLength(1);
    expect(calls[0].command).toBe("plugin:probe|probe_echo");
    expect(calls[0].args).toEqual({ payload: "hello" });
  });

  it("conf 槽位的 invoke 同样命中 plugin:probe| 前缀", () => {
    expect(probeConfSlot.invoke).toBe("plugin:probe|get_probe_conf");
  });
});

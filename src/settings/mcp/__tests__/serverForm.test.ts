import { describe, expect, it } from "vitest";
import type { McpServerDto } from "@/types/protocol.contract";
import {
  buildLaunch,
  formatArgs,
  isLaunchDirty,
  maskEnvKeys,
  parseArgs,
  parseEnv,
  requiresEnvRefill,
  statusLabelKey,
  statusTagColor,
  type LaunchDraft,
} from "../serverForm";

const SERVER: McpServerDto = {
  id: "filesystem",
  name: "filesystem",
  transport: "stdio",
  enabled: true,
  status: "ready",
  lastError: null,
  command: "npx",
  args: ["-y", "@modelcontextprotocol/server-filesystem"],
  cwd: null,
  envKeys: [],
  callTimeoutMs: null,
  configError: null,
  createdAt: 0,
  updatedAt: 0,
};

function draft(overrides: Partial<LaunchDraft> = {}): LaunchDraft {
  return {
    command: SERVER.command,
    argsText: formatArgs(SERVER.args),
    cwd: "",
    envText: "",
    ...overrides,
  };
}

describe("parseArgs / formatArgs", () => {
  it("splits one argument per line and drops blanks", () => {
    expect(parseArgs("  -y \n\n  foo  \n")).toEqual(["-y", "foo"]);
  });

  it("round-trips through formatArgs", () => {
    expect(parseArgs(formatArgs(["a", "b c"]))).toEqual(["a", "b c"]);
  });
});

describe("parseEnv", () => {
  it("splits on the first equals sign and keeps the rest verbatim", () => {
    expect(parseEnv("A=1\nTOKEN=abc=def\n").env).toEqual({
      A: "1",
      TOKEN: "abc=def",
    });
  });

  it("collects lines that are not KEY=VALUE", () => {
    expect(parseEnv("A=1\nbroken\n=no-key\n").invalidLines).toEqual([
      "broken",
      "=no-key",
    ]);
  });

  it("ignores blank lines", () => {
    expect(parseEnv("\n\n").env).toEqual({});
  });
});

describe("maskEnvKeys", () => {
  it("shows keys with a fixed mask and never the values", () => {
    expect(maskEnvKeys(["API_KEY", "TOKEN"])).toBe(
      "API_KEY=••••••  TOKEN=••••••",
    );
  });
});

describe("buildLaunch", () => {
  it("trims command, drops empty cwd and parses env", () => {
    const launch = buildLaunch(
      draft({ command: "  node  ", cwd: "  ", envText: "A=1" }),
    );
    expect(launch.command).toBe("node");
    expect(launch.cwd).toBeNull();
    expect(launch.env).toEqual({ A: "1" });
  });
});

describe("isLaunchDirty", () => {
  it("is false when only the name changes", () => {
    expect(isLaunchDirty(draft(), SERVER)).toBe(false);
  });

  it("detects command, args and cwd changes", () => {
    expect(isLaunchDirty(draft({ command: "node" }), SERVER)).toBe(true);
    expect(isLaunchDirty(draft({ argsText: "-y" }), SERVER)).toBe(true);
    expect(isLaunchDirty(draft({ cwd: "/tmp" }), SERVER)).toBe(true);
  });

  it("treats freshly typed env as a change", () => {
    expect(isLaunchDirty(draft({ envText: "A=1" }), SERVER)).toBe(true);
  });
});

describe("requiresEnvRefill", () => {
  const withEnv: McpServerDto = { ...SERVER, envKeys: ["API_KEY"] };

  it("blocks a launch change while env values are unavailable", () => {
    expect(requiresEnvRefill(draft({ command: "node" }), withEnv)).toBe(true);
  });

  it("does not block when the launch config is untouched", () => {
    expect(requiresEnvRefill(draft(), withEnv)).toBe(false);
  });

  it("does not block once env is re-entered", () => {
    expect(
      requiresEnvRefill(
        draft({ command: "node", envText: "API_KEY=1" }),
        withEnv,
      ),
    ).toBe(false);
  });

  it("does not block servers without env", () => {
    expect(requiresEnvRefill(draft({ command: "node" }), SERVER)).toBe(false);
  });
});

describe("status helpers", () => {
  it("maps runtime status to an i18n key", () => {
    expect(statusLabelKey("ready")).toBe("settings.mcp.status.ready");
  });

  it("maps runtime status to a tag color", () => {
    expect(statusTagColor("ready")).toBe("green");
    expect(statusTagColor("starting")).toBe("blue");
    expect(statusTagColor("error")).toBe("red");
    expect(statusTagColor("stopped")).toBe("default");
  });
});

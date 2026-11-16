import { describe, it, expect } from "vitest";
import {
  stripVerbatimPrefix,
  workspaceBasename,
  workspaceDisplay,
} from "../workspaceDisplay";
import type { WorkspaceEntry } from "../../../services/desktop/workspaceService";

const entries: WorkspaceEntry[] = [
  { path: "G:\\code\\alpha", name: "alpha", missing: false },
  { path: "G:\\code\\gone", name: "gone", missing: true },
];

describe("stripVerbatimPrefix", () => {
  it("strips the verbatim drive prefix", () => {
    expect(stripVerbatimPrefix("\\\\?\\G:\\code\\alpha")).toBe(
      "G:\\code\\alpha",
    );
  });

  it("rewrites the verbatim UNC prefix", () => {
    expect(stripVerbatimPrefix("\\\\?\\UNC\\server\\share")).toBe(
      "\\\\server\\share",
    );
  });

  it("leaves plain paths untouched", () => {
    expect(stripVerbatimPrefix("G:\\code\\alpha")).toBe("G:\\code\\alpha");
    expect(stripVerbatimPrefix("/home/user/alpha")).toBe("/home/user/alpha");
  });
});

describe("workspaceBasename", () => {
  it("returns last segment for Windows and POSIX paths", () => {
    expect(workspaceBasename("G:\\code\\alpha")).toBe("alpha");
    expect(workspaceBasename("/home/user/alpha")).toBe("alpha");
  });

  it("ignores trailing separators", () => {
    expect(workspaceBasename("G:\\code\\alpha\\")).toBe("alpha");
    expect(workspaceBasename("/home/user/alpha/")).toBe("alpha");
  });

  it("works on verbatim paths", () => {
    expect(workspaceBasename("\\\\?\\G:\\code\\alpha")).toBe("alpha");
  });
});

describe("workspaceDisplay", () => {
  it("treats empty string as unconfigured (never null)", () => {
    expect(workspaceDisplay("", entries)).toEqual({
      state: "unconfigured",
      name: "",
      path: "",
      displayPath: "",
    });
  });

  it("uses directory book name when entry exists", () => {
    expect(workspaceDisplay("G:\\code\\alpha", entries)).toEqual({
      state: "valid",
      name: "alpha",
      path: "G:\\code\\alpha",
      displayPath: "G:\\code\\alpha",
    });
  });

  it("flags missing when the entry is probed missing", () => {
    expect(workspaceDisplay("G:\\code\\gone", entries).state).toBe("missing");
  });

  it("falls back to basename when the entry is absent", () => {
    expect(workspaceDisplay("G:\\code\\beta", entries)).toEqual({
      state: "valid",
      name: "beta",
      path: "G:\\code\\beta",
      displayPath: "G:\\code\\beta",
    });
  });

  it("keeps the stored path verbatim but shows the stripped path", () => {
    const verbatim = "\\\\?\\G:\\code\\beta";
    const display = workspaceDisplay(verbatim, entries);
    expect(display.path).toBe(verbatim);
    expect(display.displayPath).toBe("G:\\code\\beta");
    expect(display.name).toBe("beta");
  });
});

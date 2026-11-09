import { describe, it, expect, vi } from "vitest";
import { switchWorkspace } from "../workspaceSwitch";

const PATH = "G:\\code\\alpha";

describe("switchWorkspace", () => {
  it("registers before patching, in that order", async () => {
    const calls: string[] = [];
    const register = vi.fn(() => {
      calls.push("register");
      return Promise.resolve({});
    });
    const patch = vi.fn(() => {
      calls.push("patch");
      return Promise.resolve(true);
    });

    const result = await switchWorkspace({ register, patch }, PATH);

    expect(result).toEqual({ kind: "switched" });
    expect(calls).toEqual(["register", "patch"]);
    expect(register).toHaveBeenCalledWith(PATH);
    expect(patch).toHaveBeenCalledWith(PATH);
  });

  it("skips the patch when registration fails", async () => {
    const error = new Error("path is not a directory");
    const register = vi.fn(() => Promise.reject(error));
    const patch = vi.fn(() => Promise.resolve(true));

    const result = await switchWorkspace({ register, patch }, PATH);

    expect(result).toEqual({ kind: "register_failed", error });
    expect(patch).not.toHaveBeenCalled();
  });

  it("reports patch failure without pretending success", async () => {
    const register = vi.fn(() => Promise.resolve({}));
    const patch = vi.fn(() => Promise.resolve(false));

    const result = await switchWorkspace({ register, patch }, PATH);

    expect(result).toEqual({ kind: "patch_failed", error: undefined });
    expect(register).toHaveBeenCalledWith(PATH);
  });

  it("converts a thrown patch error into patch_failed", async () => {
    const error = new Error("busy");
    const register = vi.fn(() => Promise.resolve({}));
    const patch = vi.fn(() => Promise.reject(error));

    const result = await switchWorkspace({ register, patch }, PATH);

    expect(result).toEqual({ kind: "patch_failed", error });
  });
});

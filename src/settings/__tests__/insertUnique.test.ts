import { describe, expect, it } from "vitest";
import { insertUnique } from "@/settings/core/insertUnique";

function setup() {
  const list: string[] = [];
  const keys = new Set<string>();
  return { list, keys };
}

describe("insertUnique", () => {
  it("pushes a new entry and records its key", () => {
    const { list, keys } = setup();

    expect(insertUnique(list, keys, "a", "a", "setting key")).toBe(true);
    expect(list).toEqual(["a"]);
    expect(keys.has("a")).toBe(true);
  });

  it("keeps distinct keys in insertion order", () => {
    const { list, keys } = setup();

    insertUnique(list, keys, "a", "a", "setting key");
    insertUnique(list, keys, "b", "b", "setting key");

    expect(list).toEqual(["a", "b"]);
  });

  it("throws on a duplicate key in DEV and leaves the list untouched", () => {
    const { list, keys } = setup();
    insertUnique(list, keys, "a", "a", "setting key");

    expect(() => insertUnique(list, keys, "a", "b", "setting key")).toThrow(
      "[settings] duplicate setting key: a",
    );
    expect(list).toEqual(["a"]);
  });
});

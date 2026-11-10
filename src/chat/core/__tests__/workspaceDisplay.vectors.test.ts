import { describe, expect, it } from "vitest";
import { readRepoFile } from "@/test-utils/paths";

import { stripVerbatimPrefix } from "../workspaceDisplay";

interface Vector {
  input: string;
  expected: string;
  note?: string;
}

const vectors: Vector[] = (
  JSON.parse(
    readRepoFile("src-tauri/tests/fixtures/verbatim-form-vectors.json"),
  ) as { vectors: Vector[] }
).vectors;

describe("stripVerbatimPrefix 共用向量（I44 §10.2）", () => {
  it("向量文件非空", () => {
    expect(vectors.length).toBeGreaterThan(0);
  });

  it.each(
    vectors.map((v) => [v.note ?? v.input, v.input, v.expected] as const),
  )("%s", (_note, input, expected) => {
    expect(stripVerbatimPrefix(input)).toBe(expected);
  });
});

import { describe, expect, it } from "vitest";
import { COLLAPSIBLE_GROUPS, buildBlocks } from "../entryColumns";
import { entryRowView, type EntryRowView } from "../entryView";
import { makeEntry } from "./fixtures";

const CORE_KEYS = [
  "attachments",
  "attachments/derived",
  "tool-results",
  "tool-index",
  "mascot-assets",
  "assistant.db",
  "logs",
  "crashes",
  "EBWebView",
  "config.json",
  "orb-config.json",
  "mascot-config.json",
];

const GROUP = COLLAPSIBLE_GROUPS[0];

const SECTION_KEYS = [1, 2, 3, 4, 5, 6].map((n) => `${GROUP}/k${n}`);

function rowsOf(keys: string[]): EntryRowView[] {
  return keys.map((key) => entryRowView(makeEntry({ key })));
}

describe("条目区块：行数与分组完全来自 entries 夹具", () => {
  it("18 条（全挂载）：core 12 条平铺 + 分区 6 条并入可折叠组", () => {
    const keys = [...CORE_KEYS, ...SECTION_KEYS];
    const rows = rowsOf(keys);
    const blocks = buildBlocks(rows);

    expect(rows.map((row) => row.key)).toEqual(keys);
    expect(rows.every((row) => row.name.length > 0)).toBe(true);
    expect(blocks.flatMap((block) => block.rows)).toHaveLength(keys.length);
    expect(blocks.every((block) => block.rows.length > 0)).toBe(true);

    const flat = blocks.filter((block) => block.group === null);
    const grouped = blocks.filter((block) => block.group !== null);
    expect(flat).toHaveLength(CORE_KEYS.length);
    expect(grouped).toHaveLength(1);
    expect(grouped[0].group).toBe(GROUP);
    expect(grouped[0].rows.map((row) => row.key)).toEqual(SECTION_KEYS);
  });

  it("12 条（未挂载）：整组不出现，也不补占位行", () => {
    const keys = [...CORE_KEYS];
    const rows = rowsOf(keys);
    const blocks = buildBlocks(rows);

    expect(rows.map((row) => row.key)).toEqual(keys);
    expect(blocks.flatMap((block) => block.rows)).toHaveLength(
      CORE_KEYS.length,
    );
    expect(blocks.every((block) => block.group === null)).toBe(true);
    expect(blocks.every((block) => block.rows.length > 0)).toBe(true);
  });

  it("空 entries：不渲染任何区块", () => {
    const blocks = buildBlocks(rowsOf([]));

    expect(blocks).toEqual([]);
  });
});

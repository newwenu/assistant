import { describe, it, expect } from "vitest";
import {
  buildHistoryTree,
  computeFocusPath,
  flattenHistoryRows,
  resolveDefaultFocusLeaf,
  resolveFocusLeafId,
  ROOT_GROUP_ID,
  type HistoryRow,
} from "../historyTree";
import type { Message } from "../../../chat/core/chatTypes";
import { makeMessage } from "../../../test-utils/messageFixtures";

const noExpand = new Set<string>();
const flatten = (
  messages: Message[],
  focusLeafId: string | null,
  expanded: ReadonlySet<string> = noExpand,
  rawRows = false,
) =>
  flattenHistoryRows(buildHistoryTree(messages), focusLeafId, expanded, {
    rawRows,
  });

const messageIds = (rows: HistoryRow[]): string[] =>
  rows
    .filter((row) => row.kind === "message")
    .map((row) => (row.kind === "message" ? row.node.message.id : ""));

describe("buildHistoryTree", () => {
  it("T1: builds a linear chain with increasing depth", () => {
    const messages = [
      makeMessage("u1", { timestamp: 100 }),
      makeMessage("a1", { role: "assistant", parentId: "u1", timestamp: 200 }),
      makeMessage("u2", { parentId: "a1", timestamp: 300 }),
      makeMessage("a2", { role: "assistant", parentId: "u2", timestamp: 400 }),
    ];
    const tree = buildHistoryTree(messages);
    expect(tree.roots).toHaveLength(1);
    expect(tree.orphanRoots).toHaveLength(0);
    expect(tree.nodeById.get("u1")!.depth).toBe(0);
    expect(tree.nodeById.get("a1")!.depth).toBe(1);
    expect(tree.nodeById.get("a2")!.depth).toBe(3);
    expect(tree.nodeById.get("u1")!.subtreeSize).toBe(4);

    const rows = flattenHistoryRows(tree, "a2", noExpand, { rawRows: false });
    expect(rows).toHaveLength(4);
    expect(rows.every((row) => row.kind === "message")).toBe(true);
    expect(messageIds(rows)).toEqual(["u1", "a1", "u2", "a2"]);
  });

  it("T9: sorts siblings by siblingIndex then timestamp then id", () => {
    const messages = [
      makeMessage("root", { timestamp: 100 }),
      makeMessage("b", {
        parentId: "root",
        siblingIndex: 1,
        siblingCount: 3,
        timestamp: 300,
      }),
      makeMessage("a", {
        parentId: "root",
        siblingIndex: 0,
        siblingCount: 3,
        timestamp: 200,
      }),
      makeMessage("c", {
        parentId: "root",
        siblingIndex: 1,
        siblingCount: 3,
        timestamp: 400,
      }),
    ];
    const tree = buildHistoryTree(messages);
    expect(
      tree.nodeById.get("root")!.children.map((node) => node.message.id),
    ).toEqual(["a", "b", "c"]);
  });

  it("T6: unattached messages are grouped by their missing parent id", () => {
    const messages = [
      makeMessage("u1", { timestamp: 100 }),
      makeMessage("ghost", { parentId: "missing-id", timestamp: 200 }),
      makeMessage("ghost2", { parentId: "missing-id-2", timestamp: 300 }),
    ];
    const tree = buildHistoryTree(messages);
    expect(tree.orphanRoots.map((node) => node.message.id)).toEqual([
      "ghost",
      "ghost2",
    ]);
    expect(tree.nodeById.get("ghost")!.orphan).toBe(true);
    expect(
      tree.orphanGroups.get("missing-id")!.map((node) => node.message.id),
    ).toEqual(["ghost"]);
    expect(
      tree.orphanGroups.get("missing-id-2")!.map((node) => node.message.id),
    ).toEqual(["ghost2"]);

    const rows = flattenHistoryRows(tree, null, noExpand, { rawRows: false });
    expect(rows.filter((row) => row.kind === "stub")).toHaveLength(0);
    const orphanRows = rows.filter(
      (row) => row.kind === "message" && row.dimmed,
    );
    expect(orphanRows).toHaveLength(2);
  });

  it("T6b: orphans sharing one missing parent form one stub group", () => {
    const messages = [
      makeMessage("u1", { timestamp: 100 }),
      makeMessage("g1", {
        parentId: "missing-id",
        siblingIndex: 0,
        siblingCount: 2,
        timestamp: 200,
      }),
      makeMessage("g2", {
        parentId: "missing-id",
        siblingIndex: 1,
        siblingCount: 2,
        timestamp: 300,
      }),
      makeMessage("other", { parentId: "missing-other", timestamp: 400 }),
    ];
    const tree = buildHistoryTree(messages);

    const rows = flattenHistoryRows(tree, "g1", noExpand, { rawRows: false });
    const stubs = rows.filter((row) => row.kind === "stub");
    expect(stubs).toHaveLength(1);
    if (stubs[0].kind === "stub") {
      expect(stubs[0].key).toBe("stub:missing-id");
      expect(stubs[0].orphan).toBe(true);
      expect(stubs[0].siblings.map((node) => node.message.id)).toEqual(["g2"]);
    }
  });

  it("T7: cycle is downgraded to orphan without infinite loop", () => {
    const messages = [
      makeMessage("u1", { timestamp: 100 }),
      makeMessage("x", { parentId: "y", timestamp: 200 }),
      makeMessage("y", { parentId: "x", timestamp: 300 }),
    ];
    const tree = buildHistoryTree(messages);
    const rows = flattenHistoryRows(tree, null, noExpand, { rawRows: false });
    expect(
      tree.nodeById.get("x")!.orphan || tree.nodeById.get("y")!.orphan,
    ).toBe(true);
    expect(rows.length).toBeLessThanOrEqual(3);
    expect(messageIds(rows).length).toBeGreaterThan(0);
  });
});

describe("flattenHistoryRows", () => {
  const branchMessages = (): Message[] => [
    makeMessage("u1", { timestamp: 100 }),
    makeMessage("a1", { role: "assistant", parentId: "u1", timestamp: 200 }),
    makeMessage("a1b", {
      role: "assistant",
      parentId: "u1",
      siblingIndex: 1,
      siblingCount: 2,
      timestamp: 500,
    }),
    makeMessage("u2", { parentId: "a1", timestamp: 300 }),
    makeMessage("a2", { role: "assistant", parentId: "u2", timestamp: 400 }),
  ];

  it("T3: non-focus sibling collapses into one stub row", () => {
    const messages = branchMessages();
    messages[1].siblingIndex = 0;
    const tree = buildHistoryTree(messages);
    const rows = flattenHistoryRows(tree, "a2", noExpand, { rawRows: false });

    const stubs = rows.filter((row) => row.kind === "stub");
    expect(stubs).toHaveLength(1);
    expect(messageIds(rows)).toEqual(["u1", "a1", "u2", "a2"]);
    const stub = stubs[0];
    if (stub.kind === "stub") {
      expect(stub.parentId).toBe("u1");
      expect(stub.siblings.map((node) => node.message.id)).toEqual(["a1b"]);
      expect(stub.expanded).toBe(false);
    }
  });

  it("T4: expanding a stub emits the side branch subtree as dimmed rows", () => {
    const messages = branchMessages();
    messages[1].siblingIndex = 0;
    messages.push(makeMessage("u3", { parentId: "a1b", timestamp: 600 }));
    const tree = buildHistoryTree(messages);
    const rows = flattenHistoryRows(tree, "a2", new Set(["stub:u1"]), {
      rawRows: false,
    });

    const ids = messageIds(rows);
    expect(ids).toEqual(["u1", "a1b", "u3", "a1", "u2", "a2"]);
    const dimmedIds = rows
      .filter((row) => row.kind === "message" && row.dimmed)
      .map((row) => (row.kind === "message" ? row.node.message.id : ""));
    expect(dimmedIds).toEqual(["a1b", "u3"]);
  });

  it("T5: invalid focus falls back to deepest leaf", () => {
    const messages = branchMessages();
    messages[1].siblingIndex = 0;
    const tree = buildHistoryTree(messages);
    expect(resolveFocusLeafId(tree, "nope")).toBeNull();
    expect(resolveDefaultFocusLeaf(tree, null)).toBe("a2");
    expect(resolveDefaultFocusLeaf(tree, "a1b")).toBe("a1b");
  });

  it("T8: raw row mode emits tool rows", () => {
    const messages = [
      makeMessage("u1", { timestamp: 100 }),
      makeMessage("a1", { role: "assistant", parentId: "u1", timestamp: 200 }),
      makeMessage("t1", { role: "tool", parentId: "a1", timestamp: 300 }),
      makeMessage("a2", { role: "assistant", parentId: "t1", timestamp: 400 }),
    ];
    const normal = flatten(messages, "a2");
    expect(messageIds(normal)).toEqual(["u1", "a1", "a2"]);

    const raw = flatten(messages, "a2", noExpand, true);
    expect(messageIds(raw)).toEqual(["u1", "a1", "t1", "a2"]);
  });

  it("T2: tool rows are transparent so iterations share indent", () => {
    const messages = [
      makeMessage("u1", { timestamp: 100 }),
      makeMessage("a1", { role: "assistant", parentId: "u1", timestamp: 200 }),
      makeMessage("t1", { role: "tool", parentId: "a1", timestamp: 250 }),
      makeMessage("a2", { role: "assistant", parentId: "t1", timestamp: 300 }),
      makeMessage("t2", { role: "tool", parentId: "a2", timestamp: 350 }),
      makeMessage("a3", { role: "assistant", parentId: "t2", timestamp: 400 }),
    ];
    const tree = buildHistoryTree(messages);
    const rows = flattenHistoryRows(tree, "a3", noExpand, { rawRows: false });
    expect(messageIds(rows)).toEqual(["u1", "a1", "a2", "a3"]);
    const indents = rows.map((row) => row.indent);
    expect(indents).toEqual([0, 0, 0, 0]);
  });

  it("T10: compact_summary renders as a normal node", () => {
    const messages = [
      makeMessage("u1", { timestamp: 100 }),
      makeMessage("s1", {
        role: "compact_summary",
        parentId: "u1",
        timestamp: 200,
      }),
      makeMessage("u2", { parentId: "s1", timestamp: 300 }),
    ];
    const rows = flatten(messages, "u2");
    expect(messageIds(rows)).toEqual(["u1", "s1", "u2"]);
  });

  it("keeps root group id stable and ignores empty input", () => {
    expect(flatten([], null)).toEqual([]);
    const rows = flatten([makeMessage("u1")], "u1");
    expect(rows).toHaveLength(1);
    expect(rows[0].indent).toBe(0);
    expect(
      computeFocusPath(buildHistoryTree([makeMessage("u1")]), "u1"),
    ).toEqual(new Set(["u1"]));
    expect(ROOT_GROUP_ID).toBe("__root__");
  });
});

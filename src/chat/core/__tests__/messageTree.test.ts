import { describe, it, expect } from "vitest";
import { MessageTree } from "../messageTree";
import type { Message } from "../chatTypes";
import { makeMessage } from "../../../test-utils/messageFixtures";

describe("MessageTree.fromActiveBranch", () => {
  it("builds tree from linear message sequence", () => {
    const messages: Message[] = [
      makeMessage("u1", { role: "user", timestamp: 100 }),
      makeMessage("a1", { role: "assistant", parentId: "u1", timestamp: 200 }),
      makeMessage("u2", { role: "user", parentId: "a1", timestamp: 300 }),
    ];

    const tree = MessageTree.fromActiveBranch(messages);
    expect(tree.getActiveLeafId()).toBe("u2");
    expect(tree.getSiblingCount("u1")).toBe(1);
    expect(tree.getSiblingCount("a1")).toBe(1);
  });

  it("handles empty message array", () => {
    const tree = MessageTree.fromActiveBranch([]);
    expect(tree.getActiveLeafId()).toBeNull();
    expect(tree.getSiblings("any")).toBeNull();
  });

  it("handles single message", () => {
    const messages = [makeMessage("u1", { role: "user" })];
    const tree = MessageTree.fromActiveBranch(messages);
    expect(tree.getActiveLeafId()).toBe("u1");
    expect(tree.getSiblings("u1")).toBeNull();
  });
});

describe("MessageTree.getSiblings", () => {
  it("returns null for single-branch message", () => {
    const messages = [
      makeMessage("u1", { role: "user", siblingCount: 1 }),
      makeMessage("a1", { role: "assistant", parentId: "u1", siblingCount: 1 }),
    ];
    const tree = MessageTree.fromActiveBranch(messages);
    expect(tree.getSiblings("a1")).toBeNull();
  });

  it("returns sibling info for multi-branch message", () => {
    const messages = [
      makeMessage("u1", { role: "user", siblingCount: 1 }),
      makeMessage("a1", {
        role: "assistant",
        parentId: "u1",
        siblingIndex: 0,
        siblingCount: 3,
      }),
    ];
    const tree = MessageTree.fromActiveBranch(messages);
    const siblings = tree.getSiblings("a1");
    expect(siblings).not.toBeNull();
    expect(siblings!.length).toBe(3);
    expect(siblings![0]).toEqual({ index: 0, isCurrent: true });
    expect(siblings![1]).toEqual({ index: 1, isCurrent: false });
    expect(siblings![2]).toEqual({ index: 2, isCurrent: false });
  });

  it("returns null for nonexistent message", () => {
    const tree = MessageTree.fromActiveBranch([
      makeMessage("u1", { role: "user" }),
    ]);
    expect(tree.getSiblings("ghost")).toBeNull();
  });

  it("defaults siblingCount to 1 when undefined", () => {
    const messages = [makeMessage("u1", { role: "user" })];
    const tree = MessageTree.fromActiveBranch(messages);
    expect(tree.getSiblingCount("u1")).toBe(1);
    expect(tree.getSiblings("u1")).toBeNull();
  });
});

describe("MessageTree.getPathToRoot", () => {
  it("traces path from leaf to root", () => {
    const messages: Message[] = [
      makeMessage("u1", { role: "user", timestamp: 100 }),
      makeMessage("a1", { role: "assistant", parentId: "u1", timestamp: 200 }),
      makeMessage("u2", { role: "user", parentId: "a1", timestamp: 300 }),
      makeMessage("a2", { role: "assistant", parentId: "u2", timestamp: 400 }),
    ];
    const tree = MessageTree.fromActiveBranch(messages);
    const path = tree.getPathToRoot("a2");
    expect(path.map((m) => m.id)).toEqual(["u1", "a1", "u2", "a2"]);
  });

  it("returns single element for root message", () => {
    const messages = [makeMessage("u1", { role: "user" })];
    const tree = MessageTree.fromActiveBranch(messages);
    const path = tree.getPathToRoot("u1");
    expect(path.map((m) => m.id)).toEqual(["u1"]);
  });

  it("returns empty array for nonexistent message", () => {
    const tree = MessageTree.fromActiveBranch([
      makeMessage("u1", { role: "user" }),
    ]);
    expect(tree.getPathToRoot("ghost")).toEqual([]);
  });

  it("stops at orphan parentId (broken chain)", () => {
    const messages: Message[] = [
      makeMessage("a1", {
        role: "assistant",
        parentId: "missing-parent",
        timestamp: 200,
      }),
    ];
    const tree = MessageTree.fromActiveBranch(messages);
    const path = tree.getPathToRoot("a1");
    expect(path.map((m) => m.id)).toEqual(["a1"]);
  });
});

describe("MessageTree.getActiveLeafId", () => {
  it("returns the only leaf in linear chain", () => {
    const messages: Message[] = [
      makeMessage("u1", { role: "user", timestamp: 100 }),
      makeMessage("a1", { role: "assistant", parentId: "u1", timestamp: 200 }),
    ];
    const tree = MessageTree.fromActiveBranch(messages);
    expect(tree.getActiveLeafId()).toBe("a1");
  });

  it("returns null for empty tree", () => {
    const tree = MessageTree.fromActiveBranch([]);
    expect(tree.getActiveLeafId()).toBeNull();
  });

  it("picks a leaf when multiple candidates exist (branch point)", () => {
    const messages: Message[] = [
      makeMessage("u1", { role: "user", timestamp: 100 }),
      makeMessage("a1", {
        role: "assistant",
        parentId: "u1",
        siblingIndex: 0,
        siblingCount: 2,
        timestamp: 200,
      }),
    ];
    const tree = MessageTree.fromActiveBranch(messages);
    const leaf = tree.getActiveLeafId();
    expect(leaf).toBe("a1");
  });
});

describe("MessageTree.getSiblingCount", () => {
  it("returns stored siblingCount", () => {
    const messages = [
      makeMessage("a1", { role: "assistant", siblingCount: 5 }),
    ];
    const tree = MessageTree.fromActiveBranch(messages);
    expect(tree.getSiblingCount("a1")).toBe(5);
  });

  it("defaults to 1 for unknown message", () => {
    const tree = MessageTree.fromActiveBranch([]);
    expect(tree.getSiblingCount("ghost")).toBe(1);
  });
});

describe("MessageTree.getTurnRootId", () => {
  it("returns the assistant itself when its parent is a user message", () => {
    const messages: Message[] = [
      makeMessage("u1", { role: "user", timestamp: 100 }),
      makeMessage("a1", { role: "assistant", parentId: "u1", timestamp: 200 }),
    ];
    const tree = MessageTree.fromActiveBranch(messages);
    expect(tree.getTurnRootId("a1")).toBe("a1");
  });

  it("walks up an agent tool chain to the turn's first assistant row", () => {
    const messages: Message[] = [
      makeMessage("u1", { role: "user", timestamp: 100 }),
      makeMessage("a1", { role: "assistant", parentId: "u1", timestamp: 200 }),
      makeMessage("t1", { role: "tool", parentId: "a1", timestamp: 300 }),
      makeMessage("a2", { role: "assistant", parentId: "t1", timestamp: 400 }),
      makeMessage("t2", { role: "tool", parentId: "a2", timestamp: 500 }),
      makeMessage("a3", { role: "assistant", parentId: "t2", timestamp: 600 }),
    ];
    const tree = MessageTree.fromActiveBranch(messages);
    expect(tree.getTurnRootId("a3")).toBe("a1");
    expect(tree.getTurnRootId("a2")).toBe("a1");
  });

  it("stops at the user message of the current turn, not the previous one", () => {
    const messages: Message[] = [
      makeMessage("u1", { role: "user", timestamp: 100 }),
      makeMessage("a1", { role: "assistant", parentId: "u1", timestamp: 200 }),
      makeMessage("u2", { role: "user", parentId: "a1", timestamp: 300 }),
      makeMessage("a2", { role: "assistant", parentId: "u2", timestamp: 400 }),
      makeMessage("t1", { role: "tool", parentId: "a2", timestamp: 500 }),
      makeMessage("a3", { role: "assistant", parentId: "t1", timestamp: 600 }),
    ];
    const tree = MessageTree.fromActiveBranch(messages);
    expect(tree.getTurnRootId("a3")).toBe("a2");
  });

  it("returns the input id for unknown message", () => {
    const tree = MessageTree.fromActiveBranch([
      makeMessage("u1", { role: "user" }),
    ]);
    expect(tree.getTurnRootId("ghost")).toBe("ghost");
  });

  it("keeps an orphan assistant row as its own turn root", () => {
    const messages: Message[] = [
      makeMessage("a1", {
        role: "assistant",
        parentId: "missing",
        timestamp: 100,
      }),
    ];
    const tree = MessageTree.fromActiveBranch(messages);
    expect(tree.getTurnRootId("a1")).toBe("a1");
  });
});

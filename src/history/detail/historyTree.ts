import type { Message } from "../../chat/core/chatTypes";

export const BRANCH_COLORS = [
  "#1677ff",
  "#52c41a",
  "#faad14",
  "#ff4d4f",
  "#722ed1",
  "#13c2c2",
  "#eb2f96",
  "#fa541c",
];

export const ROOT_GROUP_ID = "__root__";
export const ORPHAN_GROUP_ID = "__orphan__";

export interface HistoryNode {
  message: Message;
  children: HistoryNode[];
  depth: number;
  subtreeSize: number;
  deepestLeafId: string;
  orphan: boolean;
}

export interface HistoryTree {
  roots: HistoryNode[];
  orphanRoots: HistoryNode[];
  /** 孤儿按「缺失的父 id」分组，保证 `BranchPicker` 的兄弟语义与后端一致。 */
  orphanGroups: Map<string, HistoryNode[]>;
  nodeById: Map<string, HistoryNode>;
}

export interface HistoryMessageRow {
  kind: "message";
  key: string;
  node: HistoryNode;
  indent: number;
  lane: number;
  dimmed: boolean;
  onFocusPath: boolean;
  /** 是否渲染身份行（头像 + 模式 + 模型）；由 `applyIdentityPass` 派生。 */
  showIdentity: boolean;
}

export interface HistoryStubRow {
  kind: "stub";
  key: string;
  parentId: string;
  indent: number;
  lane: number;
  siblings: HistoryNode[];
  expanded: boolean;
  orphan: boolean;
}

export type HistoryRow = HistoryMessageRow | HistoryStubRow;

export interface HistoryFlattenOptions {
  /** 是否把 `role === "tool"` 的链式行渲染为独立行。 */
  showToolRows: boolean;
}

function compareSiblings(a: HistoryNode, b: HistoryNode): number {
  const byIndex = a.message.siblingIndex - b.message.siblingIndex;
  if (byIndex !== 0) return byIndex;
  const byTime = a.message.timestamp - b.message.timestamp;
  if (byTime !== 0) return byTime;
  if (a.message.id === b.message.id) return 0;
  return a.message.id < b.message.id ? -1 : 1;
}

function sortSiblings(nodes: HistoryNode[]): void {
  nodes.sort(compareSiblings);
}

/**
 * 全量消息 → 多叉树。
 *
 * 与 chat 侧的 `MessageTree` 职责不同：这里保留所有角色行（含 `tool` /
 * `compact_summary`），因为 Agent 链是 `assistant → tool → assistant` 的
 * 链式存储，按角色过滤会让下一轮 assistant 失去父节点而变成孤儿。
 */
export function buildHistoryTree(messages: Message[]): HistoryTree {
  const nodeById = new Map<string, HistoryNode>();
  for (const message of messages) {
    nodeById.set(message.id, {
      message,
      children: [],
      depth: 0,
      subtreeSize: 1,
      deepestLeafId: message.id,
      orphan: false,
    });
  }

  const childrenOf = new Map<string, HistoryNode[]>();
  const roots: HistoryNode[] = [];

  for (const message of messages) {
    const node = nodeById.get(message.id)!;
    const parentId = message.parentId;
    const parent = parentId ? nodeById.get(parentId) : undefined;
    if (parent) {
      let siblings = childrenOf.get(parentId!);
      if (!siblings) {
        siblings = [];
        childrenOf.set(parentId!, siblings);
      }
      siblings.push(node);
    } else if (!parentId) {
      roots.push(node);
    }
  }

  for (const siblings of childrenOf.values()) sortSiblings(siblings);
  sortSiblings(roots);

  for (const node of nodeById.values()) {
    node.children = childrenOf.get(node.message.id) ?? [];
  }

  const visited = new Set<string>();
  const finalize = (entry: HistoryNode[]) => {
    const stack: Array<{ node: HistoryNode; phase: 0 | 1; depth: number }> = [];
    for (let i = entry.length - 1; i >= 0; i--) {
      stack.push({ node: entry[i], phase: 0, depth: 0 });
    }
    while (stack.length > 0) {
      const item = stack.pop()!;
      const node = item.node;
      if (item.phase === 0) {
        if (visited.has(node.message.id)) continue;
        visited.add(node.message.id);
        node.depth = item.depth;
        stack.push({ node, phase: 1, depth: item.depth });
        for (let i = node.children.length - 1; i >= 0; i--) {
          stack.push({
            node: node.children[i],
            phase: 0,
            depth: item.depth + 1,
          });
        }
      } else {
        let size = 1;
        let bestId = node.message.id;
        let bestDepth = node.depth;
        let bestIsTool = node.message.role === "tool";
        let bestTime = node.message.timestamp;
        for (const child of node.children) {
          size += child.subtreeSize;
          const leaf = nodeById.get(child.deepestLeafId);
          const leafDepth = leaf ? leaf.depth : child.depth;
          const leafIsTool = leaf ? leaf.message.role === "tool" : false;
          const leafTime = leaf
            ? leaf.message.timestamp
            : child.message.timestamp;
          const deeper = leafDepth > bestDepth;
          const sameDepthBetter =
            leafDepth === bestDepth && bestIsTool && !leafIsTool;
          const sameDepthNewer =
            leafDepth === bestDepth &&
            bestIsTool === leafIsTool &&
            leafTime > bestTime;
          if (deeper || sameDepthBetter || sameDepthNewer) {
            bestId = child.deepestLeafId;
            bestDepth = leafDepth;
            bestIsTool = leafIsTool;
            bestTime = leafTime;
          }
        }
        node.subtreeSize = size;
        node.deepestLeafId = bestId;
      }
    }
  };
  finalize(roots);

  const orphanRoots: HistoryNode[] = [];
  const orphanGroups = new Map<string, HistoryNode[]>();
  for (const message of messages) {
    const node = nodeById.get(message.id);
    if (!node || visited.has(message.id)) continue;
    const parentId = message.parentId;
    const parent = parentId ? nodeById.get(parentId) : undefined;
    if (parent) {
      parent.children = parent.children.filter(
        (child) => child.message.id !== message.id,
      );
    }
    node.orphan = true;
    orphanRoots.push(node);
    const groupKey = parentId ?? ORPHAN_GROUP_ID;
    const group = orphanGroups.get(groupKey);
    if (group) group.push(node);
    else orphanGroups.set(groupKey, [node]);
    finalize([node]);
  }
  sortSiblings(orphanRoots);
  for (const group of orphanGroups.values()) sortSiblings(group);

  return { roots, orphanRoots, orphanGroups, nodeById };
}

export function computeFocusPath(
  tree: HistoryTree,
  focusLeafId: string | null,
): Set<string> {
  const path = new Set<string>();
  let id: string | null = focusLeafId;
  while (id) {
    const node = tree.nodeById.get(id);
    if (!node || path.has(id)) break;
    path.add(id);
    id = node.message.parentId;
  }
  return path;
}

/** 任意节点 id → 其子树最深叶（优先非 `tool` 行，便于渲染出可见气泡）。 */
export function resolveFocusLeafId(
  tree: HistoryTree,
  nodeId: string | null,
): string | null {
  if (!nodeId) return null;
  const node = tree.nodeById.get(nodeId);
  if (!node) return null;
  return node.deepestLeafId;
}

/**
 * 默认焦点：优先会话的 `activeBranchId`；否则取最后一个根的最深叶。
 */
export function resolveDefaultFocusLeaf(
  tree: HistoryTree,
  activeBranchId: string | null,
): string | null {
  if (activeBranchId && tree.nodeById.has(activeBranchId)) {
    return resolveFocusLeafId(tree, activeBranchId);
  }
  const groups = tree.roots.length > 0 ? tree.roots : tree.orphanRoots;
  if (groups.length === 0) return null;
  const last = groups[groups.length - 1];
  return last.deepestLeafId;
}

/**
 * 身份行归并（I51 §2.2）：同一 turn 内仅首个非旁支 assistant 行渲染身份行。
 *
 * 用输出后处理而非递归内传递状态，原因有二：可见序列会被 stub 行隔开，
 * 「上一行是否同 turn」不可用；旁支展开的 `dimmed` 行不应抢占该 turn 的身份行。
 * `turnIndex === null` 的存量行逐行显示，禁止用相邻关系或时间戳推断轮次。
 */
function applyIdentityPass(rows: HistoryRow[]): void {
  const seenTurn = new Set<number>();
  for (const row of rows) {
    if (row.kind !== "message") continue;
    const message = row.node.message;
    if (message.role !== "assistant") {
      row.showIdentity = false;
      continue;
    }
    if (message.turnIndex == null) {
      row.showIdentity = true;
      continue;
    }
    if (row.dimmed) {
      row.showIdentity = false;
      continue;
    }
    row.showIdentity = !seenTurn.has(message.turnIndex);
    seenTurn.add(message.turnIndex);
  }
}

export function flattenHistoryRows(
  tree: HistoryTree,
  focusLeafId: string | null,
  expandedKeys: ReadonlySet<string>,
  opts: HistoryFlattenOptions,
): HistoryRow[] {
  const rows: HistoryRow[] = [];
  const showToolRows = opts.showToolRows;
  const effectiveFocusId =
    focusLeafId && tree.nodeById.has(focusLeafId)
      ? focusLeafId
      : resolveDefaultFocusLeaf(tree, null);
  const focusPath = computeFocusPath(tree, effectiveFocusId);

  const laneOf = (node: HistoryNode, parentLane: number): number => {
    const count = node.message.siblingCount ?? 1;
    return count > 1 ? node.message.siblingIndex : parentLane;
  };

  const pickPreferred = (children: HistoryNode[]): HistoryNode => {
    for (const child of children) {
      if (focusPath.has(child.message.id)) return child;
    }
    let best = children[0];
    let bestDepth = -1;
    let bestTime = -Infinity;
    for (const child of children) {
      const leaf = tree.nodeById.get(child.deepestLeafId);
      const depth = leaf ? leaf.depth : child.depth;
      if (
        depth > bestDepth ||
        (depth === bestDepth && child.message.timestamp >= bestTime)
      ) {
        best = child;
        bestDepth = depth;
        bestTime = child.message.timestamp;
      }
    }
    return best;
  };

  const emitGroup = (
    parentId: string,
    children: HistoryNode[],
    indent: number,
    parentLane: number,
    parentDimmed: boolean,
    orphan = false,
  ): void => {
    if (children.length === 0) return;
    const preferred = pickPreferred(children);
    const others = children.filter((child) => child !== preferred);

    if (others.length > 0) {
      const key = `stub:${parentId}`;
      const expanded = expandedKeys.has(key);
      rows.push({
        kind: "stub",
        key,
        parentId,
        indent,
        lane: parentLane,
        siblings: others,
        expanded,
        orphan,
      });
      if (expanded) {
        for (const other of others) {
          emitNode(other, indent + 1, laneOf(other, parentLane), true);
        }
      }
    }

    emitNode(
      preferred,
      indent,
      laneOf(preferred, parentLane),
      parentDimmed || !focusPath.has(preferred.message.id),
    );
  };

  const emitNode = (
    node: HistoryNode,
    indent: number,
    lane: number,
    dimmed: boolean,
  ): void => {
    if (node.message.role === "tool" && !showToolRows) {
      emitGroup(node.message.id, node.children, indent, lane, dimmed);
      return;
    }
    rows.push({
      kind: "message",
      key: `message:${node.message.id}`,
      node,
      indent,
      lane,
      dimmed,
      onFocusPath: focusPath.has(node.message.id),
      showIdentity: false,
    });
    emitGroup(node.message.id, node.children, indent, lane, dimmed);
  };

  emitGroup(ROOT_GROUP_ID, tree.roots, 0, 0, false);
  for (const [groupKey, group] of tree.orphanGroups) {
    emitGroup(groupKey, group, 0, 0, true, true);
  }

  applyIdentityPass(rows);
  return rows;
}

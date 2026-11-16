import type { Message } from "../chatTypes";

export interface SiblingInfo {
  index: number;
  isCurrent: boolean;
}

interface MessageTreeNode {
  message: Message;
  children: string[];
  siblingIndex: number;
  siblingCount: number;
}

export class MessageTree {
  private nodes: Map<string, MessageTreeNode> = new Map();

  static fromActiveBranch(messages: Message[]): MessageTree {
    const tree = new MessageTree();
    for (const msg of messages) {
      tree.nodes.set(msg.id, {
        message: msg,
        children: [],
        siblingIndex: msg.siblingIndex,
        siblingCount: msg.siblingCount ?? 1,
      });
    }
    for (const msg of messages) {
      if (msg.parentId && tree.nodes.has(msg.parentId)) {
        tree.nodes.get(msg.parentId)!.children.push(msg.id);
      }
    }
    return tree;
  }

  getSiblings(messageId: string): SiblingInfo[] | null {
    const node = this.nodes.get(messageId);
    if (!node || node.siblingCount <= 1) return null;
    return Array.from({ length: node.siblingCount }, (_, i) => ({
      index: i,
      isCurrent: i === node.siblingIndex,
    }));
  }

  getPathToRoot(messageId: string): Message[] {
    const path: Message[] = [];
    let current = this.nodes.get(messageId);
    while (current) {
      path.unshift(current.message);
      current = current.message.parentId
        ? this.nodes.get(current.message.parentId)
        : undefined;
    }
    return path;
  }

  getTurnRootId(messageId: string): string {
    let current = this.nodes.get(messageId);
    if (!current) return messageId;

    const visited = new Set<string>([current.message.id]);
    let cursor = current.message.parentId
      ? this.nodes.get(current.message.parentId)
      : undefined;
    while (cursor && !visited.has(cursor.message.id)) {
      if (cursor.message.role === "user") break;
      visited.add(cursor.message.id);
      if (cursor.message.role === "assistant") {
        current = cursor;
      }
      cursor = cursor.message.parentId
        ? this.nodes.get(cursor.message.parentId)
        : undefined;
    }
    return current.message.id;
  }

  getActiveLeafId(): string | null {
    for (const [id, node] of this.nodes) {
      if (node.children.length === 0) return id;
    }
    return null;
  }

  getSiblingCount(messageId: string): number {
    return this.nodes.get(messageId)?.siblingCount ?? 1;
  }
}

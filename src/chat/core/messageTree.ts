import type { Message } from "./chatTypes";

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

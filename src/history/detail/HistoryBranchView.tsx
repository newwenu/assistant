import React, { useMemo } from "react";
import type { Message } from "../../chat/core/chatTypes";
import {
  buildHistoryTree,
  flattenHistoryRows,
  resolveDefaultFocusLeaf,
  resolveFocusLeafId,
  type HistoryRow,
} from "./historyTree";
import { HistoryBranchRow } from "./HistoryBranchRow";
import { HistoryStubRow } from "./HistoryStubRow";
import { HistoryVirtualList } from "./HistoryVirtualList";

interface HistoryBranchViewProps {
  messages: Message[];
  activeBranchId: string | null;
  focusLeafId: string | null;
  expandedKeys: ReadonlySet<string>;
  rawRows: boolean;
  scrollTop: number;
  viewportHeight: number;
  scrollContainerRef: React.RefObject<HTMLDivElement | null>;
  revealNonce: number;
  onDeleteMessage: (id: string) => void;
  onFocusNode: (nodeId: string) => void;
  onToggleStub: (key: string) => void;
}

const rowKey = (row: HistoryRow): string => row.key;

export const HistoryBranchView: React.FC<HistoryBranchViewProps> = ({
  messages,
  activeBranchId,
  focusLeafId,
  expandedKeys,
  rawRows,
  scrollTop,
  viewportHeight,
  scrollContainerRef,
  revealNonce,
  onDeleteMessage,
  onFocusNode,
  onToggleStub,
}) => {
  const tree = useMemo(() => buildHistoryTree(messages), [messages]);

  const effectiveFocusLeafId = useMemo(() => {
    if (focusLeafId && tree.nodeById.has(focusLeafId)) {
      return resolveFocusLeafId(tree, focusLeafId);
    }
    return resolveDefaultFocusLeaf(tree, activeBranchId);
  }, [tree, focusLeafId, activeBranchId]);

  const rows = useMemo(
    () =>
      flattenHistoryRows(tree, effectiveFocusLeafId, expandedKeys, { rawRows }),
    [tree, effectiveFocusLeafId, expandedKeys, rawRows],
  );

  const renderRow = React.useCallback(
    (row: HistoryRow) => {
      if (row.kind === "stub") {
        return (
          <HistoryStubRow
            row={row}
            onToggleStub={onToggleStub}
            onFocusNode={onFocusNode}
          />
        );
      }
      return (
        <HistoryBranchRow
          row={row}
          isFocusLeaf={row.node.message.id === effectiveFocusLeafId}
          onDeleteMessage={onDeleteMessage}
          onFocusNode={onFocusNode}
        />
      );
    },
    [effectiveFocusLeafId, onDeleteMessage, onFocusNode, onToggleStub],
  );

  return (
    <HistoryVirtualList
      rows={rows}
      rowKey={rowKey}
      renderRow={renderRow}
      scrollTop={scrollTop}
      viewportHeight={viewportHeight}
      scrollContainerRef={scrollContainerRef}
      revealKey={
        revealNonce > 0 && focusLeafId ? `message:${focusLeafId}` : null
      }
      revealNonce={revealNonce}
    />
  );
};

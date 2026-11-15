import React, { useMemo } from "react";
import type { Message } from "../../chat/core/chatTypes";
import {
  buildHistoryTree,
  flattenHistoryRows,
  resolveDefaultFocusLeaf,
  resolveFocusLeafId,
  type HistoryRow,
} from "./historyTree";
import { HistoryBranchRow, type HistoryViewMode } from "./HistoryBranchRow";
import { HistoryStubRow } from "./HistoryStubRow";
import { HistoryVirtualList } from "./HistoryVirtualList";

interface HistoryBranchViewProps {
  messages: Message[];
  activeBranchId: string | null;
  focusLeafId: string | null;
  expandedKeys: ReadonlySet<string>;
  showToolRows: boolean;
  viewMode: HistoryViewMode;
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
  showToolRows,
  viewMode,
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
      flattenHistoryRows(tree, effectiveFocusLeafId, expandedKeys, {
        showToolRows,
      }),
    [tree, effectiveFocusLeafId, expandedKeys, showToolRows],
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
          viewMode={viewMode}
          isFocusLeaf={row.node.message.id === effectiveFocusLeafId}
          onDeleteMessage={onDeleteMessage}
          onFocusNode={onFocusNode}
        />
      );
    },
    [
      effectiveFocusLeafId,
      viewMode,
      onDeleteMessage,
      onFocusNode,
      onToggleStub,
    ],
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

import React, { useEffect, useMemo, useRef, useState } from "react";
import type { Message } from "../../chat/core/chatTypes";
import type { MessageRecordDto } from "../../types/protocol.contract";
import { historyService } from "../../services/historyService";
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
  conversationId: string;
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
  conversationId,
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

  // 落库原文缓存（I51 §6.3）：挂在常驻的列表容器上，行组件会被虚拟列表反复卸载，
  // 放行内会导致每次重挂都重新拉取。会话切换即整体丢弃。
  const [records, setRecords] = useState<ReadonlyMap<string, MessageRecordDto>>(
    () => new Map<string, MessageRecordDto>(),
  );
  const [recordsError, setRecordsError] = useState<string | null>(null);
  const loadedForRef = useRef<string | null>(null);
  const fetchSeqRef = useRef(0);

  useEffect(() => {
    loadedForRef.current = null;
    fetchSeqRef.current += 1;
    setRecords(new Map<string, MessageRecordDto>());
    setRecordsError(null);
  }, [conversationId]);

  useEffect(() => {
    if (viewMode !== "raw") return;
    if (loadedForRef.current === conversationId) return;
    const seq = fetchSeqRef.current + 1;
    fetchSeqRef.current = seq;
    loadedForRef.current = conversationId;
    let cancelled = false;
    const isStale = () => cancelled || seq !== fetchSeqRef.current;
    historyService.getMessageRecords(conversationId).then(
      (rows) => {
        if (isStale()) return;
        setRecords(new Map(rows.map((row) => [row.id, row])));
        setRecordsError(null);
      },
      (error: unknown) => {
        if (isStale()) return;
        setRecords(new Map<string, MessageRecordDto>());
        setRecordsError(error instanceof Error ? error.message : String(error));
        // 失败不留「已加载」标记：重新进入原始模式时允许再试一次，
        // 否则一次瞬时失败会让该会话的原始视图永久停在降级状态。
        loadedForRef.current = null;
      },
    );
    return () => {
      cancelled = true;
    };
  }, [conversationId, viewMode]);

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
      const messageId = row.node.message.id;
      return (
        <HistoryBranchRow
          row={row}
          viewMode={viewMode}
          record={records.get(messageId) ?? null}
          recordError={recordsError}
          isFocusLeaf={messageId === effectiveFocusLeafId}
          onDeleteMessage={onDeleteMessage}
          onFocusNode={onFocusNode}
        />
      );
    },
    [
      effectiveFocusLeafId,
      viewMode,
      records,
      recordsError,
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

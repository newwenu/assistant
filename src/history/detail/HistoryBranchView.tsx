import React, {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
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
import { HistoryBranchRow } from "./HistoryBranchRow";
import { HistoryStubRow } from "./HistoryStubRow";
import { HistoryVirtualList } from "./HistoryVirtualList";
import type { HistoryRecordState, RecordSource } from "./recordSnapshot";

interface HistoryBranchViewProps {
  conversationId: string;
  messages: Message[];
  activeBranchId: string | null;
  focusLeafId: string | null;
  expandedKeys: ReadonlySet<string>;
  showToolRows: boolean;
  scrollTop: number;
  viewportHeight: number;
  scrollContainerRef: React.RefObject<HTMLDivElement | null>;
  revealNonce: number;
  onDeleteMessage: (id: string) => void;
  onFocusNode: (nodeId: string) => void;
  onToggleStub: (key: string) => void;
}

const rowKey = (row: HistoryRow): string => row.key;

const EMPTY_RECORDS: ReadonlyMap<string, MessageRecordDto> = new Map<
  string,
  MessageRecordDto
>();

export const HistoryBranchView: React.FC<HistoryBranchViewProps> = ({
  conversationId,
  messages,
  activeBranchId,
  focusLeafId,
  expandedKeys,
  showToolRows,
  scrollTop,
  viewportHeight,
  scrollContainerRef,
  revealNonce,
  onDeleteMessage,
  onFocusNode,
  onToggleStub,
}) => {
  const tree = useMemo(() => buildHistoryTree(messages), [messages]);

  // 落库原文缓存（I51 §4.5 / §6.3）：挂在常驻的列表容器上，行组件会被虚拟列表反复卸载，
  // 放行内会导致每次重挂都重新拉取。会话切换即整体丢弃。
  const [records, setRecords] =
    useState<ReadonlyMap<string, MessageRecordDto>>(EMPTY_RECORDS);
  const [recordsError, setRecordsError] = useState<string | null>(null);
  const [recordsLoading, setRecordsLoading] = useState(false);
  const loadedForRef = useRef<string | null>(null);
  const fetchSeqRef = useRef(0);

  useEffect(() => {
    loadedForRef.current = null;
    fetchSeqRef.current += 1;
    setRecords(EMPTY_RECORDS);
    setRecordsError(null);
    setRecordsLoading(false);
  }, [conversationId]);

  // 拉取时机是「任一行首次展开」，不是进入某个视图模式（I51 §4.5）。
  const requestRecords = useCallback(() => {
    if (loadedForRef.current === conversationId) return;
    const seq = fetchSeqRef.current + 1;
    fetchSeqRef.current = seq;
    loadedForRef.current = conversationId;
    setRecordsLoading(true);
    historyService.getMessageRecords(conversationId).then(
      (rows) => {
        if (seq !== fetchSeqRef.current) return;
        setRecords(new Map(rows.map((row) => [row.id, row])));
        setRecordsError(null);
        setRecordsLoading(false);
      },
      (error: unknown) => {
        if (seq !== fetchSeqRef.current) return;
        setRecords(EMPTY_RECORDS);
        setRecordsError(error instanceof Error ? error.message : String(error));
        setRecordsLoading(false);
        // 失败不留「已加载」标记：再次展开时允许重试，
        // 否则一次瞬时失败会让该会话永久停在降级态。
        loadedForRef.current = null;
      },
    );
  }, [conversationId]);

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

  const renderRow = useCallback(
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
      const record = records.get(messageId) ?? null;
      const source: RecordSource = recordsLoading
        ? "loading"
        : record
          ? "raw"
          : "dto";
      const recordState: HistoryRecordState = {
        source,
        record,
        error: recordsLoading ? null : recordsError,
      };
      return (
        <HistoryBranchRow
          row={row}
          recordState={recordState}
          isFocusLeaf={messageId === effectiveFocusLeafId}
          onDeleteMessage={onDeleteMessage}
          onFocusNode={onFocusNode}
          onRequestRecords={requestRecords}
        />
      );
    },
    [
      effectiveFocusLeafId,
      records,
      recordsLoading,
      recordsError,
      requestRecords,
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

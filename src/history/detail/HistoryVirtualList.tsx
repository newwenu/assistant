import React, {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import {
  VIRTUAL_SCROLL_OVERSCAN,
  VIRTUAL_SCROLL_THRESHOLD,
} from "../../config/constants";

const ESTIMATED_ROW_HEIGHT = 96;

interface HistoryVirtualListProps<T> {
  rows: T[];
  rowKey: (row: T) => string;
  renderRow: (row: T) => React.ReactNode;
  scrollTop: number;
  viewportHeight: number;
  scrollContainerRef?: React.RefObject<HTMLDivElement | null>;
  revealKey?: string | null;
  revealNonce?: number;
  overscan?: number;
  threshold?: number;
}

interface MeasuredRowProps {
  offset: number;
  onMeasure: (key: string, height: number) => void;
  rowKey: string;
  children: React.ReactNode;
}

const MeasuredRow: React.FC<MeasuredRowProps> = ({
  offset,
  onMeasure,
  rowKey,
  children,
}) => {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const element = ref.current;
    if (!element) return;
    const observer = new ResizeObserver((entries) => {
      const height = entries[0]?.contentRect.height;
      if (height != null) onMeasure(rowKey, height);
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, [onMeasure, rowKey]);

  return (
    <div
      ref={ref}
      data-row-key={rowKey}
      style={{ position: "absolute", top: offset, left: 0, right: 0 }}
    >
      {children}
    </div>
  );
};

function findIndex(offsets: number[], target: number): number {
  let low = 0;
  let high = offsets.length - 1;
  while (low < high) {
    const mid = Math.floor((low + high) / 2);
    if (offsets[mid] <= target) low = mid + 1;
    else high = mid;
  }
  return Math.max(0, low - 1);
}

export function HistoryVirtualList<T>({
  rows,
  rowKey,
  renderRow,
  scrollTop,
  viewportHeight,
  scrollContainerRef,
  revealKey = null,
  revealNonce = 0,
  overscan = VIRTUAL_SCROLL_OVERSCAN,
  threshold = VIRTUAL_SCROLL_THRESHOLD,
}: HistoryVirtualListProps<T>): React.ReactElement {
  const heightsRef = useRef(new Map<string, number>());
  const [measureVersion, setMeasureVersion] = useState(0);

  const onMeasure = useCallback((key: string, height: number) => {
    const previous = heightsRef.current.get(key);
    const next = Math.round(height);
    if (previous != null && Math.abs(previous - next) < 1) return;
    heightsRef.current.set(key, next);
    setMeasureVersion((value) => value + 1);
  }, []);

  const offsets = useMemo(() => {
    void measureVersion;
    const result = new Array<number>(rows.length + 1);
    result[0] = 0;
    for (let index = 0; index < rows.length; index += 1) {
      const key = rowKey(rows[index]);
      result[index + 1] =
        result[index] + (heightsRef.current.get(key) ?? ESTIMATED_ROW_HEIGHT);
    }
    return result;
  }, [rows, rowKey, measureVersion]);

  const virtualized = rows.length > threshold;

  const lastRevealNonceRef = useRef(-1);

  useEffect(() => {
    if (!revealKey || revealNonce === lastRevealNonceRef.current) return;
    const container = scrollContainerRef?.current;
    if (!container) return;
    const index = rows.findIndex((row) => rowKey(row) === revealKey);
    if (index < 0) return;
    lastRevealNonceRef.current = revealNonce;

    const findRowElement = (): HTMLElement | undefined =>
      Array.from(
        container.querySelectorAll<HTMLElement>("[data-row-key]"),
      ).find((candidate) => candidate.dataset.rowKey === revealKey);

    const centerRow = (): void => {
      const element = findRowElement();
      if (!element) return;
      const rowRect = element.getBoundingClientRect();
      const containerRect = container.getBoundingClientRect();
      const margin = 8;
      const visible =
        rowRect.top >= containerRect.top + margin &&
        rowRect.bottom <= containerRect.bottom - margin;
      if (visible) return;
      container.scrollTop +=
        rowRect.top -
        containerRect.top -
        (container.clientHeight - rowRect.height) / 2;
    };

    if (findRowElement()) {
      centerRow();
      return;
    }

    if (virtualized) {
      const top = offsets[index];
      const height = offsets[index + 1] - top;
      container.scrollTop = Math.max(
        0,
        top - (container.clientHeight - height) / 2,
      );
    }

    requestAnimationFrame(() => {
      requestAnimationFrame(centerRow);
    });
  }, [
    revealKey,
    revealNonce,
    rows,
    offsets,
    rowKey,
    scrollContainerRef,
    virtualized,
  ]);

  if (!virtualized) {
    return (
      <>
        {rows.map((row) => {
          const key = rowKey(row);
          return (
            <div key={key} data-row-key={key}>
              {renderRow(row)}
            </div>
          );
        })}
      </>
    );
  }

  const totalHeight = offsets[rows.length];
  const start = Math.max(0, findIndex(offsets, scrollTop) - overscan);
  const end = Math.min(
    rows.length,
    findIndex(offsets, scrollTop + viewportHeight) + 1 + overscan,
  );

  return (
    <div style={{ height: totalHeight, position: "relative" }}>
      {rows.slice(start, end).map((row, index) => {
        const key = rowKey(row);
        return (
          <MeasuredRow
            key={key}
            rowKey={key}
            offset={offsets[start + index]}
            onMeasure={onMeasure}
          >
            {renderRow(row)}
          </MeasuredRow>
        );
      })}
    </div>
  );
}

import {
  useEffect,
  useRef,
  useState,
  type PointerEvent as ReactPointerEvent,
} from "react";

import {
  DEFAULT_COLUMN_WIDTHS,
  clampColumnWidth,
  type ColumnKey,
} from "./columns";

interface DragState {
  key: ColumnKey;
  startX: number;
  startWidth: number;
}

/**
 * 列宽拖拽：`pointerdown` 记录起点，`pointermove` 实时改宽，`pointerup` /
 * `pointercancel` 结束。拖拽期间把 `body` 光标锁成 `col-resize` 并禁用选中，
 * 避免指针移出轨道后选中文本。
 */
export function useColumnResize() {
  const [columnWidths, setColumnWidths] = useState(DEFAULT_COLUMN_WIDTHS);
  const [resizing, setResizing] = useState<ColumnKey | null>(null);
  const dragRef = useRef<DragState | null>(null);

  useEffect(() => {
    if (resizing === null) return;
    const onMove = (event: PointerEvent) => {
      const drag = dragRef.current;
      if (drag === null) return;
      const next = clampColumnWidth(
        drag.startWidth + (event.clientX - drag.startX),
      );
      setColumnWidths((prev) =>
        prev[drag.key] === next ? prev : { ...prev, [drag.key]: next },
      );
    };
    const onEnd = () => {
      dragRef.current = null;
      setResizing(null);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onEnd);
    window.addEventListener("pointercancel", onEnd);
    return () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onEnd);
      window.removeEventListener("pointercancel", onEnd);
    };
  }, [resizing]);

  useEffect(() => {
    if (resizing === null) return;
    const previousCursor = document.body.style.cursor;
    const previousUserSelect = document.body.style.userSelect;
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
    return () => {
      document.body.style.cursor = previousCursor;
      document.body.style.userSelect = previousUserSelect;
    };
  }, [resizing]);

  const startResize = (
    key: ColumnKey,
    event: ReactPointerEvent<HTMLDivElement>,
  ) => {
    if (event.button !== 0) return;
    event.preventDefault();
    event.stopPropagation();
    dragRef.current = {
      key,
      startX: event.clientX,
      startWidth: columnWidths[key],
    };
    setResizing(key);
  };

  const resetColumnWidth = (key: ColumnKey) => {
    setColumnWidths((prev) =>
      prev[key] === DEFAULT_COLUMN_WIDTHS[key]
        ? prev
        : { ...prev, [key]: DEFAULT_COLUMN_WIDTHS[key] },
    );
  };

  return { columnWidths, resizing, startResize, resetColumnWidth };
}

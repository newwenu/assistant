import React, {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { useTranslation } from "react-i18next";
import type { ScreenshotSessionDto } from "@/types/protocol.contract";
import { useScreenshotStore } from "@/stores/screenshotStore";
import { useAttachmentStore } from "@/stores/attachmentStore";
import { useOrbStore } from "@/stores/orbStore";
import { useOrbExpand } from "@/orb/OrbExpand";
import { attachmentFromImport } from "@/chat/attachment/factory";
import { notifyAttachError } from "@/chat/attachment/notify";
import {
  clampToBounds,
  isValidSelection,
  normalizeDrag,
  resizeByHandle,
  toNormalizedRect,
  type HandleId,
  type Rect,
} from "./selection";
import {
  notifyScreenshotError,
  requestChatInputFocus,
  screenshotService,
} from "./session";
import css from "./Overlay.module.css";

/** 手柄抓取区 18px（I35 §14.1）：视觉点 8px，命中区更大，避免缩放时反复脱手。 */
const HANDLE_GRAB_PX = 18;
const HANDLE_DOT_PX = 8;

const HANDLES: HandleId[] = ["nw", "n", "ne", "e", "se", "s", "sw", "w"];

const HANDLE_CURSOR: Record<HandleId, string> = {
  nw: "nwse-resize",
  n: "ns-resize",
  ne: "nesw-resize",
  e: "ew-resize",
  se: "nwse-resize",
  s: "ns-resize",
  sw: "nesw-resize",
  w: "ew-resize",
};

type DragState =
  | { mode: "create"; startX: number; startY: number }
  | {
      mode: "resize";
      handle: HandleId;
      origin: Rect;
      startX: number;
      startY: number;
    };

interface OverlayProps {
  session: ScreenshotSessionDto;
}

function handleCenter(rect: Rect, handle: HandleId): { x: number; y: number } {
  const x = handle.includes("w")
    ? rect.x
    : handle.includes("e")
      ? rect.x + rect.width
      : rect.x + rect.width / 2;
  const y = handle.includes("n")
    ? rect.y
    : handle.includes("s")
      ? rect.y + rect.height
      : rect.y + rect.height / 2;
  return { x, y };
}

export const ScreenshotOverlay: React.FC<OverlayProps> = ({ session }) => {
  const { t } = useTranslation();
  const finishing = useScreenshotStore((s) => s.finishing);
  const { expand } = useOrbExpand();

  const rootRef = useRef<HTMLDivElement>(null);
  const imgRef = useRef<HTMLImageElement>(null);
  const dragRef = useRef<DragState | null>(null);
  const selectionRef = useRef<Rect | null>(null);
  const busyRef = useRef(false);

  const [revealed, setRevealed] = useState(false);
  const [selection, setSelection] = useState<Rect | null>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });
  const [busy, setBusy] = useState(false);

  selectionRef.current = selection;

  useEffect(() => {
    const root = rootRef.current;
    if (!root) return;
    const measure = () => {
      const rect = root.getBoundingClientRect();
      setSize({ width: rect.width, height: rect.height });
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(root);
    return () => observer.disconnect();
  }, []);

  const cancel = useCallback(async () => {
    if (busyRef.current) return;
    const current = useScreenshotStore.getState().session;
    if (!current) return;
    busyRef.current = true;
    setBusy(true);
    useScreenshotStore.getState().beginFinishing();
    try {
      await screenshotService.cancel(current.token);
      useScreenshotStore.getState().clearSession();
    } catch (error) {
      useScreenshotStore.getState().endFinishing();
      notifyScreenshotError(error);
      busyRef.current = false;
      setBusy(false);
    }
  }, []);

  /** 底图解码成功才揭示：没解码完就显形会露出被拉成全屏的旧 UI（§9.4）。 */
  useEffect(() => {
    const img = imgRef.current;
    if (!img) return;
    let cancelled = false;
    void img
      .decode()
      .then(() => {
        if (!cancelled) setRevealed(true);
      })
      .catch(() => {
        if (cancelled) return;
        setRevealed(true);
        notifyScreenshotError("screenshot.err.capture_failed");
        /** data URL 不该解码失败；真失败时没有可裁剪的底图，直接取消会话。 */
        void cancel();
      });
    return () => {
      cancelled = true;
    };
  }, [cancel, session.preview]);

  const confirm = useCallback(async () => {
    if (busyRef.current) return;
    const current = useScreenshotStore.getState().session;
    const rect = selectionRef.current;
    if (!current || !rect || !isValidSelection(rect)) return;
    busyRef.current = true;
    setBusy(true);
    useScreenshotStore.getState().beginFinishing();
    try {
      const result = await screenshotService.crop({
        token: current.token,
        ...toNormalizedRect(rect, size),
      });
      useScreenshotStore.getState().clearSession();
      const ingested = await attachmentFromImport(result, {
        width: result.width,
        height: result.height,
      });
      if (!ingested.ok) {
        notifyAttachError(ingested.error);
      } else {
        const error = useAttachmentStore
          .getState()
          .addAttachment(ingested.attachment);
        if (error) notifyAttachError(error);
      }
      if (!useOrbStore.getState().isExpanded) {
        await expand();
      }
      requestChatInputFocus();
    } catch (error) {
      useScreenshotStore.getState().endFinishing();
      notifyScreenshotError(error);
      busyRef.current = false;
      setBusy(false);
    }
  }, [expand, size]);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        void cancel();
        return;
      }
      if (e.key === "Enter") {
        e.preventDefault();
        void confirm();
      }
    };
    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, [cancel, confirm]);

  const bounds = useMemo<Rect>(
    () => ({ x: 0, y: 0, width: size.width, height: size.height }),
    [size.width, size.height],
  );

  const localPoint = useCallback(
    (clientX: number, clientY: number): { x: number; y: number } => {
      const rect = rootRef.current?.getBoundingClientRect();
      return { x: clientX - (rect?.left ?? 0), y: clientY - (rect?.top ?? 0) };
    },
    [],
  );

  const onPointerDown = useCallback(
    (e: React.PointerEvent) => {
      if (busyRef.current || e.button !== 0) return;
      e.preventDefault();
      const point = localPoint(e.clientX, e.clientY);
      rootRef.current?.setPointerCapture(e.pointerId);
      dragRef.current = { mode: "create", startX: point.x, startY: point.y };
      setSelection({ x: point.x, y: point.y, width: 0, height: 0 });
    },
    [localPoint],
  );

  const onHandlePointerDown = useCallback(
    (handle: HandleId) => (e: React.PointerEvent) => {
      if (busyRef.current || e.button !== 0) return;
      const rect = selectionRef.current;
      if (!rect) return;
      e.preventDefault();
      e.stopPropagation();
      const point = localPoint(e.clientX, e.clientY);
      rootRef.current?.setPointerCapture(e.pointerId);
      dragRef.current = {
        mode: "resize",
        handle,
        origin: rect,
        startX: point.x,
        startY: point.y,
      };
    },
    [localPoint],
  );

  const onPointerMove = useCallback(
    (e: React.PointerEvent) => {
      const drag = dragRef.current;
      if (!drag || busyRef.current) return;
      const point = localPoint(e.clientX, e.clientY);
      if (drag.mode === "create") {
        setSelection(
          clampToBounds(
            normalizeDrag(drag.startX, drag.startY, point.x, point.y),
            bounds,
          ),
        );
        return;
      }
      setSelection(
        resizeByHandle(
          drag.origin,
          drag.handle,
          point.x - drag.startX,
          point.y - drag.startY,
          bounds,
        ),
      );
    },
    [bounds, localPoint],
  );

  const onPointerUp = useCallback((e: React.PointerEvent) => {
    if (!dragRef.current) return;
    dragRef.current = null;
    try {
      rootRef.current?.releasePointerCapture(e.pointerId);
    } catch {
      /* 捕获可能已被浏览器释放 */
    }
    setSelection((current) =>
      current && isValidSelection(current) ? current : null,
    );
  }, []);

  const onDoubleClick = useCallback(
    (e: React.MouseEvent) => {
      const rect = selectionRef.current;
      if (!rect) return;
      const point = localPoint(e.clientX, e.clientY);
      if (
        point.x >= rect.x &&
        point.x <= rect.x + rect.width &&
        point.y >= rect.y &&
        point.y <= rect.y + rect.height
      ) {
        void confirm();
      }
    },
    [confirm, localPoint],
  );

  const onContextMenu = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      void cancel();
    },
    [cancel],
  );

  if (finishing) {
    /**
     * 提交后窗口几何正在被 Rust 还原，此时底图已无意义；保持透明而不是黑屏，
     * 还原瞬间不会在缩小的 Orb 窗口里闪一下黑色。
     */
    return (
      <div
        ref={rootRef}
        className={css.rootFinishing}
        data-screenshot-overlay
      />
    );
  }

  const mask = selection;
  const labelStyle: React.CSSProperties | null = selection
    ? {
        left: selection.x,
        top: Math.max(0, selection.y - 26),
      }
    : null;
  /** 标注用物理像素（与产物尺寸一致）：预览是逻辑分辨率，比值就是缩放比。 */
  const labelSize = selection
    ? {
        width: Math.round(
          selection.width * (session.previewWidth / Math.max(1, size.width)),
        ),
        height: Math.round(
          selection.height * (session.previewHeight / Math.max(1, size.height)),
        ),
      }
    : null;

  return (
    <div
      ref={rootRef}
      className={css.root}
      data-screenshot-overlay
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={onPointerUp}
      onDoubleClick={onDoubleClick}
      onContextMenu={onContextMenu}
    >
      <img
        ref={imgRef}
        className={`${css.image}${revealed ? "" : ` ${css.imageHidden}`}`}
        src={session.preview}
        alt=""
        draggable={false}
      />
      <svg
        className={css.svg}
        width={size.width}
        height={size.height}
        viewBox={`0 0 ${Math.max(1, size.width)} ${Math.max(1, size.height)}`}
      >
        {mask ? (
          <>
            <rect
              x={0}
              y={0}
              width={size.width}
              height={Math.max(0, mask.y)}
              className={css.mask}
            />
            <rect
              x={0}
              y={mask.y + mask.height}
              width={size.width}
              height={Math.max(0, size.height - mask.y - mask.height)}
              className={css.mask}
            />
            <rect
              x={0}
              y={mask.y}
              width={Math.max(0, mask.x)}
              height={mask.height}
              className={css.mask}
            />
            <rect
              x={mask.x + mask.width}
              y={mask.y}
              width={Math.max(0, size.width - mask.x - mask.width)}
              height={mask.height}
              className={css.mask}
            />
            <rect
              x={mask.x}
              y={mask.y}
              width={mask.width}
              height={mask.height}
              className={css.selection}
            />
          </>
        ) : (
          <rect
            x={0}
            y={0}
            width={size.width}
            height={size.height}
            className={css.mask}
          />
        )}
      </svg>
      {selection && isValidSelection(selection) ? (
        <>
          <div className={css.label} style={labelStyle ?? undefined}>
            {labelSize?.width} × {labelSize?.height}
          </div>
          {HANDLES.map((handle) => {
            const center = handleCenter(selection, handle);
            return (
              <div
                key={handle}
                className={css.handle}
                style={{
                  left: center.x - HANDLE_GRAB_PX / 2,
                  top: center.y - HANDLE_GRAB_PX / 2,
                  width: HANDLE_GRAB_PX,
                  height: HANDLE_GRAB_PX,
                  cursor: HANDLE_CURSOR[handle],
                }}
                onPointerDown={onHandlePointerDown(handle)}
              >
                <span
                  className={css.handleDot}
                  style={{ width: HANDLE_DOT_PX, height: HANDLE_DOT_PX }}
                />
              </div>
            );
          })}
        </>
      ) : null}
      {revealed && !selection ? (
        <div className={css.hint}>{t("screenshot.select_hint")}</div>
      ) : null}
      {busy ? <div className={css.busy} /> : null}
    </div>
  );
};

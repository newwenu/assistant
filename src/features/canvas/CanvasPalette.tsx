import { memo, useCallback, useRef, useState } from "react";
import type { CardTone } from "./types";
import styles from "./canvas.module.css";

/** 放置栏拖拽的自定义 MIME：只有放置栏条目写入，视口据此识别落卡（A24）。 */
export const CARD_KIND_MIME = "application/x-canvas-card-kind";

/** 放置栏标题行拖动时相对画布宿主的位移（页面内，不进世界坐标）。 */
interface PaletteOffset {
  x: number;
  y: number;
}

const PALETTE_MARGIN = 12;

/** 放置栏条目：全部文案已本地化，`kind` 是画布会话里的类别标识。 */
export interface CanvasPaletteItem {
  kind: string;
  badge: string;
  name: string;
  summary: string;
  /** 类别色标：取该类别的 `kindTone`。 */
  tone: CardTone;
}

export interface CanvasPaletteProps {
  /** 放置栏标题（同时是标题行拖动柄与自身无障碍名称）。 */
  title: string;
  items: readonly CanvasPaletteItem[];
  /** 条目被点击 / 回车：请求在视口中心落一张该类别的卡片。 */
  onPlace?: (kind: string) => void;
}

/**
 * 放置栏：浮在画布左上角的竖条，只列可用类别，不参与世界坐标与几何。
 * 标题行可拖动改位置；条目可拖入画布，也可聚焦后回车在视口中心落卡。
 * 放置结果只进画布会话状态，不校验、不自动接线、不持久化（A24）。
 */
function CanvasPaletteImpl({ title, items, onPlace }: CanvasPaletteProps) {
  const [offset, setOffset] = useState<PaletteOffset>({ x: 0, y: 0 });
  const dragRef = useRef<{
    pointerId: number;
    startX: number;
    startY: number;
    origin: PaletteOffset;
    bounds: DOMRect;
  } | null>(null);

  const handleTitleDown = useCallback(
    (event: React.PointerEvent<HTMLDivElement>) => {
      if (event.button !== 0) return;
      const panel = event.currentTarget.parentElement;
      const host = panel?.offsetParent;
      if (!panel || !(host instanceof HTMLElement)) return;
      event.currentTarget.setPointerCapture(event.pointerId);
      dragRef.current = {
        pointerId: event.pointerId,
        startX: event.clientX,
        startY: event.clientY,
        origin: offset,
        bounds: host.getBoundingClientRect(),
      };
    },
    [offset],
  );

  const handleTitleMove = useCallback(
    (event: React.PointerEvent<HTMLDivElement>) => {
      const drag = dragRef.current;
      if (!drag || drag.pointerId !== event.pointerId) return;
      const panel = event.currentTarget.parentElement;
      if (!panel) return;
      const maxX = drag.bounds.width - panel.offsetWidth - PALETTE_MARGIN;
      const maxY = drag.bounds.height - panel.offsetHeight - PALETTE_MARGIN;
      setOffset({
        x: Math.min(
          Math.max(
            drag.origin.x + event.clientX - drag.startX,
            -PALETTE_MARGIN,
          ),
          Math.max(maxX, -PALETTE_MARGIN),
        ),
        y: Math.min(
          Math.max(
            drag.origin.y + event.clientY - drag.startY,
            -PALETTE_MARGIN,
          ),
          Math.max(maxY, -PALETTE_MARGIN),
        ),
      });
    },
    [],
  );

  const handleTitleUp = useCallback(
    (event: React.PointerEvent<HTMLDivElement>) => {
      const drag = dragRef.current;
      if (!drag || drag.pointerId !== event.pointerId) return;
      dragRef.current = null;
    },
    [],
  );

  return (
    <aside
      className={styles.palette}
      aria-label={title}
      style={{ transform: `translate(${offset.x}px, ${offset.y}px)` }}
    >
      <div
        className={styles.paletteHead}
        onPointerDown={handleTitleDown}
        onPointerMove={handleTitleMove}
        onPointerUp={handleTitleUp}
        onPointerCancel={handleTitleUp}
      >
        <span className={styles.paletteHeadGrip} aria-hidden="true">
          <i />
          <i />
          <i />
        </span>
        <span className={styles.paletteTitle}>{title}</span>
      </div>
      <ul className={styles.paletteList}>
        {items.map((item) => (
          <li key={item.kind}>
            <button
              type="button"
              className={styles.paletteItem}
              draggable
              aria-label={`${item.badge} · ${item.name}`}
              onDragStart={(event) => {
                event.dataTransfer.setData(CARD_KIND_MIME, item.kind);
                event.dataTransfer.effectAllowed = "copy";
              }}
              onClick={() => onPlace?.(item.kind)}
            >
              <span className={styles.paletteGrip} aria-hidden="true">
                <i />
                <i />
                <i />
              </span>
              <span className={styles.paletteItemHead}>
                <span
                  className={styles.paletteItemTone}
                  data-tone={item.tone}
                  aria-hidden="true"
                />
                <span className={styles.paletteItemBadge}>{item.badge}</span>
                <span className={styles.paletteItemName}>{item.name}</span>
              </span>
              <span className={styles.paletteItemSummary} title={item.summary}>
                {item.summary}
              </span>
            </button>
          </li>
        ))}
      </ul>
    </aside>
  );
}

export const CanvasPalette = memo(CanvasPaletteImpl);

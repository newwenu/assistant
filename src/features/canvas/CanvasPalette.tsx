import { memo } from "react";
import styles from "./canvas.module.css";

/** 放置栏拖拽的自定义 MIME：只有放置栏条目写入，视口据此识别落卡（A24）。 */
export const CARD_KIND_MIME = "application/x-canvas-card-kind";

/** 放置栏条目：全部文案已本地化，`kind` 是画布会话里的类别标识。 */
export interface CanvasPaletteItem {
  kind: string;
  badge: string;
  name: string;
  summary: string;
}

export interface CanvasPaletteProps {
  title: string;
  hint: string;
  placeHint: string;
  items: readonly CanvasPaletteItem[];
  /** 条目被点击 / 回车：请求在视口中心落一张该类别的卡片。 */
  onPlace?: (kind: string) => void;
}

/**
 * 放置栏（I68 §7.10）：页面层固定栏，只列可用类别，不参与世界坐标与几何。
 * 条目可拖入画布，也可聚焦后回车在视口中心落卡；放置结果只进画布会话状态，
 * 不校验、不自动接线、不持久化（A24）。
 */
function CanvasPaletteImpl({
  title,
  hint,
  placeHint,
  items,
  onPlace,
}: CanvasPaletteProps) {
  return (
    <aside className={styles.palette} aria-label={title}>
      <div className={styles.paletteHead}>
        <span className={styles.paletteTitle}>{title}</span>
        <span className={styles.paletteHint}>{hint}</span>
      </div>
      <ul className={styles.paletteList}>
        {items.map((item) => (
          <li key={item.kind}>
            <button
              type="button"
              className={styles.paletteItem}
              draggable
              title={placeHint}
              aria-label={`${item.badge} · ${item.name}`}
              onDragStart={(event) => {
                event.dataTransfer.setData(CARD_KIND_MIME, item.kind);
                event.dataTransfer.effectAllowed = "copy";
              }}
              onClick={() => onPlace?.(item.kind)}
            >
              <span className={styles.paletteGrip} aria-hidden="true" />
              <span className={styles.paletteItemHead}>
                <span className={styles.paletteItemBadge}>{item.badge}</span>
                <span className={styles.paletteItemName}>{item.name}</span>
              </span>
              <span className={styles.paletteItemSummary}>{item.summary}</span>
            </button>
          </li>
        ))}
      </ul>
    </aside>
  );
}

export const CanvasPalette = memo(CanvasPaletteImpl);

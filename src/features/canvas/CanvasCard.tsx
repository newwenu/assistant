import { memo } from "react";
import type { CanvasCardView } from "./types";
import styles from "./canvas.module.css";

export interface CanvasCardProps {
  card: CanvasCardView;
  selected: boolean;
  onSelect: (id: string) => void;
}

/**
 * 卡片壳（§5.2 / §9.1）：只读盒子与已本地化文案，不决定内容、不认观测字段。
 * 选中只写 `data-selected`，不改变边相位与出口（N2）。
 */
function CanvasCardImpl({ card, selected, onSelect }: CanvasCardProps) {
  const label =
    card.subtitle === undefined
      ? card.title
      : `${card.title} · ${card.subtitle}`;

  return (
    <button
      type="button"
      className={styles.card}
      data-canvas-card
      data-tone={card.tone ?? "default"}
      data-selected={selected}
      aria-pressed={selected}
      aria-label={label}
      style={{
        left: card.box.x,
        top: card.box.y,
        width: card.box.w,
        height: card.box.h,
      }}
      onClick={() => onSelect(card.id)}
    >
      <span className={styles.cardTitle}>{card.title}</span>
      {card.subtitle !== undefined && (
        <span className={styles.cardSubtitle}>{card.subtitle}</span>
      )}
      {card.badges !== undefined && card.badges.length > 0 && (
        <span className={styles.cardBadges}>
          {card.badges.map((badge, index) => (
            <span
              key={`${card.id}-badge-${index}`}
              className={styles.badge}
              data-tone={badge.tone ?? "neutral"}
            >
              {badge.text}
            </span>
          ))}
        </span>
      )}
      {card.exit !== undefined && card.exit !== null && (
        <span className={styles.exitMarker} aria-hidden="true">
          <span className={styles.exitSwatch}>
            <span className={styles.exitGlyph} data-exit={card.exit} />
          </span>
        </span>
      )}
    </button>
  );
}

export const CanvasCard = memo(CanvasCardImpl);

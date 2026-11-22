import { memo } from "react";
import { useTranslation } from "react-i18next";
import type { BadgeTone, CardTone, EdgePhase, ExitTone } from "./types";
import styles from "./canvas.module.css";

export type CanvasLegendItem =
  | { kind: "edge"; phase: EdgePhase; label: string }
  | { kind: "exit"; tone: ExitTone; label: string }
  | { kind: "card"; tone: CardTone; label: string }
  | { kind: "badge"; tone: BadgeTone; label: string };

export interface CanvasLegendGroup {
  id: string;
  title: string;
  items: readonly CanvasLegendItem[];
}

export interface CanvasLegendProps {
  showFacilityGroups?: boolean;
  groups?: readonly CanvasLegendGroup[];
}

const EDGE_PHASES: readonly EdgePhase[] = [
  "untaken",
  "idle",
  "passed",
  "active",
];

const EXIT_TONES: readonly ExitTone[] = ["failed", "cancelled", "stopped"];

/**
 * 图例条目与画布元素共用同一组 CSS 类（§10.2）：改一处即两处变，不另写图例专用样式。
 */
function LegendItemView({ item }: { item: CanvasLegendItem }) {
  switch (item.kind) {
    case "edge":
      return (
        <span className={styles.legendItem}>
          <span className={styles.edgeSwatch} data-phase={item.phase}>
            <span
              className={styles.edgeLine}
              data-phase={item.phase}
              data-orientation="horizontal"
            />
          </span>
          {item.label}
        </span>
      );
    case "exit":
      return (
        <span className={styles.legendItem}>
          <span className={styles.exitSwatch}>
            <span className={styles.exitGlyph} data-exit={item.tone} />
          </span>
          {item.label}
        </span>
      );
    case "card":
      return (
        <span className={styles.legendItem}>
          <span className={styles.cardSwatch} data-tone={item.tone} />
          {item.label}
        </span>
      );
    case "badge":
      return (
        <span className={styles.legendItem}>
          <span className={styles.badge} data-tone={item.tone}>
            {item.label}
          </span>
        </span>
      );
  }
}

function LegendGroupView({ group }: { group: CanvasLegendGroup }) {
  return (
    <div className={styles.legendGroup}>
      <span className={styles.legendTitle}>{group.title}</span>
      <div className={styles.legendItems}>
        {group.items.map((item, index) => (
          <LegendItemView key={`${group.id}-${index}`} item={item} />
        ))}
      </div>
    </div>
  );
}

/**
 * 图例（§10）：设施自带相位 / 出口两组，内容组由调用方追加；以浮层固定在视口左下，不随缩放平移。
 */
function CanvasLegendImpl({ showFacilityGroups, groups }: CanvasLegendProps) {
  const { t } = useTranslation();
  const facilityGroups: readonly CanvasLegendGroup[] =
    showFacilityGroups === false
      ? []
      : [
          {
            id: "edge-phase",
            title: t("canvas.legend.edge_phase"),
            items: EDGE_PHASES.map((phase) => ({
              kind: "edge" as const,
              phase,
              label: t(`canvas.edge.${phase}`),
            })),
          },
          {
            id: "exit",
            title: t("canvas.legend.exit"),
            items: EXIT_TONES.map((tone) => ({
              kind: "exit" as const,
              tone,
              label: t(`canvas.exit.${tone}`),
            })),
          },
        ];

  return (
    <div className={styles.legend}>
      {facilityGroups.map((group) => (
        <LegendGroupView key={group.id} group={group} />
      ))}
      {groups?.map((group) => (
        <LegendGroupView key={group.id} group={group} />
      ))}
    </div>
  );
}

export const CanvasLegend = memo(CanvasLegendImpl);

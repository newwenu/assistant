import { useTranslation } from "react-i18next";
import type { GraphNodeView } from "@/stores/agent/observation/projections/graph";
import styles from "./observation.module.css";

const ORDER_BADGES = [
  "①",
  "②",
  "③",
  "④",
  "⑤",
  "⑥",
  "⑦",
  "⑧",
  "⑨",
  "⑩",
  "⑪",
  "⑫",
] as const;

interface NodeCellProps {
  node: GraphNodeView;
  /** 已观测到的最大 `atMs`：V1 只算 `atMs` 差，不引入时间源（I55 §5.3）。 */
  nowMs: number;
  activeSource: boolean;
  activeTarget: boolean;
  /** turn 已收尾：仍停在中间态的格子标「未完成」，不臆造终态（I55 §6.1）。 */
  terminal: boolean;
}

export function NodeCell({
  node,
  nowMs,
  activeSource,
  activeTarget,
  terminal,
}: NodeCellProps) {
  const { t } = useTranslation();
  const elapsedMs = elapsedOf(node, nowMs);
  const showSource = node.source !== "observed";

  return (
    <div
      className={styles.cell}
      data-state={node.state}
      data-source={node.source}
      data-active-source={activeSource}
      data-active-target={activeTarget}
      style={{ gridColumn: node.column + 1, gridRow: node.row + 1 }}
      title={t(`observation.source.${node.source}_hint`)}
    >
      <div className={styles.cellHead}>
        <span className={styles.cellOrder}>{ORDER_BADGES[node.order]}</span>
        <span className={styles.cellName}>
          {t(`observation.node.${node.id}`)}
        </span>
      </div>
      <div className={styles.cellBadges}>
        {node.iteration > 0 && (
          <span className={styles.cellBadge}>#{node.iteration}</span>
        )}
        {showSource && (
          <span className={styles.cellBadge}>
            {t(`observation.source.${node.source}`)}
          </span>
        )}
        {terminal && node.unfinished && (
          <span className={styles.cellBadge}>
            {t("observation.graph.unfinished")}
          </span>
        )}
      </div>
      <div className={styles.cellState}>
        {t(`observation.state.${node.state}`)}
        {elapsedMs !== null && ` · ${formatSeconds(elapsedMs)}`}
      </div>
    </div>
  );
}

/**
 * 进行中时长：只用 `running` / `waiting` 的 `atMs` 差，不画百分比（I55 §5.3）。
 * `assumed` 格子的起点是猜的，不显示时长（I55 §6.6 第 4 条）。
 */
function elapsedOf(node: GraphNodeView, nowMs: number): number | null {
  if (node.source === "assumed") return null;
  if (node.state !== "running" && node.state !== "waiting") return null;
  if (nowMs <= node.sinceMs) return null;
  return nowMs - node.sinceMs;
}

function formatSeconds(ms: number): string {
  return `${(ms / 1000).toFixed(1)}s`;
}

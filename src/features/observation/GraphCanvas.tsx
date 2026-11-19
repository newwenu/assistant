import { Tag } from "antd";
import type { CSSProperties } from "react";
import { useTranslation } from "react-i18next";
import type {
  GraphNodeView,
  GraphView,
} from "@/stores/agent/observation/projections/graph";
import {
  AGENT_NODE_TEMPLATE,
  type AgentNodeTemplateEntry,
} from "@/stores/agent/observation/template";
import type { TurnObservationStatus } from "@/stores/agent/observation/types";
import { NodeCell } from "./NodeCell";
import styles from "./observation.module.css";

/**
 * 画布列数由模板推出（不写字面量，模板改了列数这里自动跟上）。
 *
 * 列宽用 `minmax(132px, 1fr)` 而不是 `minmax(0, 1fr)`：11 列挤进一屏会把每格
 * 压到 70px 左右，节点名（如「工具后中间件」「assistant 行落库」）必然截断。
 * 给出最小可读宽度，不足时由外层滚动容器横向滚动。
 */
const CANVAS_COLUMNS =
  AGENT_NODE_TEMPLATE.reduce((max, entry) => Math.max(max, entry.column), 0) +
  1;

/** 第二行（⑦ 判定之后的工具分支）在模板里只占右侧 5 列，左侧留白用行标签说明。 */
const TOOL_LANE_LABEL_COLUMNS = 6;

const canvasGridStyle: CSSProperties = {
  gridTemplateColumns: `repeat(${CANVAS_COLUMNS}, minmax(132px, 1fr))`,
};

const STATUS_TAG_COLOR: Record<TurnObservationStatus, string> = {
  running: "processing",
  completed: "success",
  stopped: "default",
  halted_by_budget: "warning",
  wrap_up: "warning",
  cancelled: "default",
  failed: "error",
};

/**
 * 12 格图（I55 §6.2）：位置来自编译期模板（O4），状态来自 `TurnObservation`。
 * 出口不是节点（I55 §3.2），只由状态徽标表达。
 */
export function GraphCanvas({ graph }: { graph: GraphView }) {
  const { t } = useTranslation();
  const terminal = graph.status !== "running";
  const activeSource = graph.activeEdge?.from ?? null;
  const activeTarget = graph.activeEdge?.to ?? null;

  return (
    <section className={styles.canvasPane}>
      <header className={styles.canvasHeader}>
        <div className={styles.canvasMeta}>
          <Tag color={STATUS_TAG_COLOR[graph.status]}>
            {t(`observation.status.${graph.status}`)}
          </Tag>
          <span className={styles.canvasMeta}>
            {t("observation.graph.iteration", { iteration: graph.iteration })}
          </span>
          {graph.activeEdge !== null && (
            <span className={styles.canvasMeta}>
              {` · ${t("observation.graph.active_edge")}: `}
              {t(`observation.node.${graph.activeEdge.from}`)}
              {" → "}
              {t(`observation.node.${graph.activeEdge.to}`)}
              {` (${t(
                graph.activeEdge.kind === "loop"
                  ? "observation.graph.loop"
                  : "observation.graph.forward",
              )})`}
            </span>
          )}
        </div>
        <span className={styles.canvasHint}>
          {t("observation.graph.no_percent")}
        </span>
      </header>
      <div className={styles.canvasScroll}>
        <div className={styles.canvas} style={canvasGridStyle}>
          <div
            className={styles.laneLabel}
            style={{
              gridColumn: `1 / span ${TOOL_LANE_LABEL_COLUMNS}`,
              gridRow: 2,
            }}
          >
            {t("observation.graph.tool_lane")}
          </div>
          {graph.nodes.map((node) => (
            <NodeCell
              key={node.id}
              node={node}
              nowMs={graph.nowMs}
              activeSource={node.id === activeSource}
              activeTarget={node.id === activeTarget}
              terminal={terminal}
            />
          ))}
        </div>
      </div>
    </section>
  );
}

/**
 * 模板预览：无观测实例时也画出 12 格布局（O4：模板是编译期常量，不依赖任何事件）。
 *
 * 全部格子是 `pending` + `source = derived`，与真实实例的推导格子同形（虚线 + 「推导」徽标），
 * 不做成「看起来在跑」的样子（§6.6 第 3 条）。
 */
export function TemplatePreview() {
  const { t } = useTranslation();

  return (
    <section className={styles.canvasPane}>
      <header className={styles.canvasHeader}>
        <span className={styles.canvasMeta}>
          {t("observation.graph.preview")}
        </span>
        <span className={styles.canvasHint}>
          {t("observation.graph.no_percent")}
        </span>
      </header>
      <div className={styles.canvasScroll}>
        <div className={styles.canvas} style={canvasGridStyle}>
          <div
            className={styles.laneLabel}
            style={{
              gridColumn: `1 / span ${TOOL_LANE_LABEL_COLUMNS}`,
              gridRow: 2,
            }}
          >
            {t("observation.graph.tool_lane")}
          </div>
          {AGENT_NODE_TEMPLATE.map((entry) => (
            <NodeCell
              key={entry.id}
              node={previewNode(entry)}
              nowMs={0}
              activeSource={false}
              activeTarget={false}
              terminal={false}
            />
          ))}
        </div>
      </div>
    </section>
  );
}

function previewNode(entry: AgentNodeTemplateEntry): GraphNodeView {
  return {
    id: entry.id,
    order: entry.order,
    row: entry.row,
    column: entry.column,
    iteration: 0,
    state: "pending",
    source: "derived",
    sinceMs: 0,
    detail: null,
    unfinished: false,
  };
}

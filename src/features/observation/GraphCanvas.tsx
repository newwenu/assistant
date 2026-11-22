import { useTranslation } from "react-i18next";
import type {
  EdgePhase,
  GraphEdgeView,
  GraphExitView,
  GraphNodeView,
  GraphView,
} from "@/stores/agent/observation/projections/graph";
import {
  AGENT_NODE_TEMPLATE,
  TEMPLATE_EDGES,
  type TemplateEdge,
} from "@/stores/agent/observation/template";
import type {
  AgentNodeId,
  NodeState,
  TurnObservationStatus,
} from "@/stores/agent/observation/types";
import styles from "./observation.module.css";

/** 固定世界坐标（不测量 DOM）：世界宽高即下方 SVG 的 viewBox 尺寸。 */
const WORLD = { width: 1180, height: 680 } as const;

/** 迭代环：③–⑫ 落在椭圆上，回边即环的最后一段弧。 */
const RING = { cx: 660, cy: 340, rx: 360, ry: 220 } as const;

/** 圈内外分界：比节点环再外扩一圈，① ② 落在圈外。 */
const BOUNDARY = { cx: 660, cy: 340, rx: 440, ry: 290 } as const;

const NODE_WIDTH = 124;
const NODE_HEIGHT = 50;

/** 相邻环节点的圆心角。 */
const STEP = 36;
/** 弧端点要离格子边界留出的空档：箭头不能压在格子上，也不能离太远。 */
const ARC_CLEARANCE = 8;
/** 裁剪搜索步长与上限：上限是半格，再退就只能画直线了。 */
const ARC_TRIM_STEP = 0.25;
const ARC_TRIM_MAX = STEP / 2 - 1;
/** ③ 刹车位于左上（自 12 点顺时针 300°），其后顺时针推进。 */
const FIRST_ANGLE = 300;

/** 圈外前置：① 在上、② 在下，② 与 ③ 同高，入口因此是「下 → 右」。 */
const ENTRY_X = 100;
const ENTRY_Y = { admit: 140, prepare: 230 } as const;

/** 每圈的起点：后端 `loop_executor` 的每轮循环第一件事就是 ③ 刹车。 */
const RING_START: AgentNodeId = "brake";

/** 环内节点顺序 = 后端 `loop_executor` 每圈的实际执行顺序（③→⑫）。 */
const RING_SEQUENCE: readonly AgentNodeId[] = [
  RING_START,
  "ladder",
  "model",
  "persist-assistant",
  "branch",
  "budget-split",
  "pipeline",
  "after-tool",
  "persist-tools",
  "after-iteration",
];

const ANGLE_BY_NODE = new Map<AgentNodeId, number>(
  RING_SEQUENCE.map((id, index) => [id, FIRST_ANGLE + index * STEP]),
);

/** 已走到的状态：用来在没有进行中格子时定位「当前停在哪个节点」。 */
const REACHED_STATES: ReadonlySet<NodeState> = new Set<NodeState>([
  "finished",
  "error",
  "cancelled",
]);

/** 图例只解释线型，不解释流程——流程由环的形状本身表达。 */
const LEGEND_PHASES: readonly EdgePhase[] = [
  "untaken",
  "idle",
  "passed",
  "active",
];

/** 出口色调：正常收尾用成功色，预算 / 停止用警告色，取消中性，失败错误色。 */
type ExitTone = "done" | "stopped" | "cancelled" | "failed";

const EXIT_TONE: Partial<Record<TurnObservationStatus, ExitTone>> = {
  completed: "done",
  wrap_up: "done",
  halted_by_budget: "stopped",
  stopped: "stopped",
  cancelled: "cancelled",
  failed: "failed",
};

interface Point {
  x: number;
  y: number;
}

interface EdgeShape {
  d: string;
  head: string;
}

/**
 * 拓扑画布（I55 §6.2）：③–⑫ 顺时针排成一圈，回边就是环的闭合弧，
 * 因此「走到 ⑫ 后回到 ③ 开始下一圈」由图形本身表达，不再需要任何文字标注。
 */
export function GraphCanvas({ graph }: { graph: GraphView }) {
  const live = currentNodeOf(graph);
  return (
    <GraphStage
      nodes={graph.nodes}
      edges={graph.edges}
      liveId={live?.id ?? null}
      exit={graph.exit}
    />
  );
}

/** 无实例时的同形预览：全部 `pending`，连线全部 `idle`。 */
export function TemplatePreview() {
  const nodes = AGENT_NODE_TEMPLATE.map((entry): GraphNodeView => ({
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
  }));
  const edges = TEMPLATE_EDGES.map((edge): GraphEdgeView => ({
    ...edge,
    phase: "idle",
  }));
  return <GraphStage nodes={nodes} edges={edges} liveId={null} exit={null} />;
}

interface GraphStageProps {
  nodes: readonly GraphNodeView[];
  edges: readonly GraphEdgeView[];
  liveId: AgentNodeId | null;
  exit: GraphExitView | null;
}

function GraphStage({ nodes, edges, liveId, exit }: GraphStageProps) {
  const { t } = useTranslation();
  const exitShape = exit === null ? null : exitStubOf(exit.node);
  const exitTone = exit === null ? null : (EXIT_TONE[exit.status] ?? null);

  return (
    <div className={styles.graphStage}>
      <div
        className={styles.graphWorld}
        style={{ width: WORLD.width, height: WORLD.height }}
      >
        <svg
          className={styles.graphSvg}
          viewBox={`0 0 ${WORLD.width} ${WORLD.height}`}
          aria-hidden="true"
        >
          <ellipse
            className={styles.graphBoundary}
            cx={BOUNDARY.cx}
            cy={BOUNDARY.cy}
            rx={BOUNDARY.rx}
            ry={BOUNDARY.ry}
          />
          {edges.map((edge) => {
            const shape = shapeOf(edge);
            return (
              <g key={`${edge.from}->${edge.to}`}>
                <path
                  className={styles.graphEdge}
                  data-phase={edge.phase}
                  data-kind={edge.kind}
                  d={shape.d}
                />
                <polygon
                  className={styles.graphArrow}
                  data-phase={edge.phase}
                  points={shape.head}
                />
              </g>
            );
          })}
          {exitShape !== null && exitTone !== null && (
            <g className={styles.graphExit} data-tone={exitTone}>
              <line
                className={styles.graphExitLine}
                x1={exitShape.start.x}
                y1={exitShape.start.y}
                x2={exitShape.end.x}
                y2={exitShape.end.y}
              />
              <rect
                className={styles.graphExitMark}
                x={exitShape.end.x - 4}
                y={exitShape.end.y - 4}
                width={8}
                height={8}
                transform={`rotate(45 ${exitShape.end.x} ${exitShape.end.y})`}
              />
            </g>
          )}
        </svg>
        {nodes.map((node) => (
          <div
            key={node.id}
            className={styles.graphNode}
            data-state={node.state}
            data-live={node.id === liveId}
            data-entry={ANGLE_BY_NODE.has(node.id) ? "false" : "true"}
            style={{
              left: centerOf(node.id).x,
              top: centerOf(node.id).y,
              width: NODE_WIDTH,
              height: NODE_HEIGHT,
            }}
            title={`${t(`observation.node.${node.id}`)} · ${t(
              `observation.state.${node.state}`,
            )}`}
          >
            <span className={styles.graphNodeHead}>
              <span className={styles.graphNodeOrdinal}>{node.order + 1}</span>
              <span className={styles.graphNodeLabel}>
                {t(`observation.short.${node.id}`)}
              </span>
            </span>
            <span className={styles.graphNodeState}>
              {t(`observation.state.${node.state}`)}
            </span>
          </div>
        ))}
        <div className={styles.graphLegend}>
          {LEGEND_PHASES.map((phase) => (
            <span
              key={phase}
              className={styles.graphLegendItem}
              data-phase={phase}
            >
              <span className={styles.graphLegendSwatch} />
              {t(`canvas.edge.${phase}`)}
            </span>
          ))}
        </div>
      </div>
    </div>
  );
}

/**
 * 当前节点：优先进行中格子，其次（本轮仍在跑时）最近走到的格子。
 * 与投影层的活动边判定同序：先比 `iteration`、再比模板 `order`。
 */
function currentNodeOf(graph: GraphView): GraphNodeView | null {
  const pick = (rows: readonly GraphNodeView[]): GraphNodeView | null => {
    let chosen: GraphNodeView | null = null;
    for (const row of rows) {
      if (
        chosen === null ||
        row.iteration > chosen.iteration ||
        (row.iteration === chosen.iteration && row.order > chosen.order)
      ) {
        chosen = row;
      }
    }
    return chosen;
  };

  const unfinished = pick(graph.nodes.filter((node) => node.unfinished));
  if (unfinished !== null) return unfinished;
  if (graph.status !== "running") return null;
  return pick(graph.nodes.filter((node) => REACHED_STATES.has(node.state)));
}

function centerOf(node: AgentNodeId): Point {
  const angle = ANGLE_BY_NODE.get(node);
  if (angle !== undefined) return ringPoint(angle);
  return { x: ENTRY_X, y: node === "admit" ? ENTRY_Y.admit : ENTRY_Y.prepare };
}

function ringPoint(angle: number): Point {
  const rad = (angle * Math.PI) / 180;
  return {
    x: RING.cx + RING.rx * Math.sin(rad),
    y: RING.cy - RING.ry * Math.cos(rad),
  };
}

function normalize(vector: Point): Point {
  const length = Math.hypot(vector.x, vector.y);
  if (length === 0) return { x: 1, y: 0 };
  return { x: vector.x / length, y: vector.y / length };
}

function arrowHead(tip: Point, direction: Point): string {
  const size = 9;
  const half = 4.5;
  const base = { x: tip.x - direction.x * size, y: tip.y - direction.y * size };
  const perp = { x: -direction.y, y: direction.x };
  const left = {
    x: base.x + perp.x * half,
    y: base.y + perp.y * half,
  };
  const right = {
    x: base.x - perp.x * half,
    y: base.y - perp.y * half,
  };
  return `${tip.x},${tip.y} ${left.x},${left.y} ${right.x},${right.y}`;
}

/** 弧端点是否已经离开格子（含空档）：格子是轴对齐矩形，判定用逐轴比较。 */
function outsideBox(point: Point, center: Point): boolean {
  return (
    Math.abs(point.x - center.x) > NODE_WIDTH / 2 + ARC_CLEARANCE ||
    Math.abs(point.y - center.y) > NODE_HEIGHT / 2 + ARC_CLEARANCE
  );
}

/** 从节点中心角出发，沿 `direction` 找到第一个已经离开格子盒子的角度。 */
function trimAngle(
  node: AgentNodeId,
  angle: number,
  direction: 1 | -1,
): number {
  const center = centerOf(node);
  for (let step = ARC_TRIM_STEP; step <= ARC_TRIM_MAX; step += ARC_TRIM_STEP) {
    const candidate = angle + direction * step;
    if (outsideBox(ringPoint(candidate), center)) return candidate;
  }
  return angle + direction * ARC_TRIM_MAX;
}

/** 环上弧段：两端都裁到格子外，`sweep=1` 即顺时针，箭头方向取端点切线。 */
function arcShape(
  from: AgentNodeId,
  fromAngle: number,
  to: AgentNodeId,
  toAngle: number,
): EdgeShape {
  const startAngle = trimAngle(from, fromAngle, 1);
  const endAngle = trimAngle(to, toAngle, -1);
  if (endAngle <= startAngle) return straightShape(from, to);
  const start = ringPoint(startAngle);
  const end = ringPoint(endAngle);
  const rad = (endAngle * Math.PI) / 180;
  const tangent = normalize({
    x: RING.rx * Math.cos(rad),
    y: RING.ry * Math.sin(rad),
  });
  return {
    d: `M ${start.x} ${start.y} A ${RING.rx} ${RING.ry} 0 0 1 ${end.x} ${end.y}`,
    head: arrowHead(end, tangent),
  };
}

/** 直线段：两端各自退到盒子边缘，避免连线穿过节点文字。 */
function straightShape(from: AgentNodeId, to: AgentNodeId): EdgeShape {
  const fromCenter = centerOf(from);
  const toCenter = centerOf(to);
  const direction = normalize({
    x: toCenter.x - fromCenter.x,
    y: toCenter.y - fromCenter.y,
  });
  const start = boxEdge(fromCenter, direction);
  const end = boxEdge(toCenter, { x: -direction.x, y: -direction.y });
  return {
    d: `M ${start.x} ${start.y} L ${end.x} ${end.y}`,
    head: arrowHead(end, direction),
  };
}

function boxEdge(center: Point, direction: Point): Point {
  const horizontal =
    direction.x === 0
      ? Number.POSITIVE_INFINITY
      : NODE_WIDTH / 2 / Math.abs(direction.x);
  const vertical =
    direction.y === 0
      ? Number.POSITIVE_INFINITY
      : NODE_HEIGHT / 2 / Math.abs(direction.y);
  const reach = Math.min(horizontal, vertical);
  return {
    x: center.x + direction.x * reach,
    y: center.y + direction.y * reach,
  };
}

/**
 * 边的几何：环内正向边是椭圆弧；回边是环的闭合弧——模板把回边登记成
 * ⑫ → ①，但 ① ② 只在 turn 开头走一次，每圈真正从 ③ 重新开始，
 * 所以回边按 ⑫ → ③ 画成闭合弧，让「一圈接一圈」直接由形状表达。
 */
function shapeOf(edge: TemplateEdge): EdgeShape {
  const fromAngle = ANGLE_BY_NODE.get(edge.from);
  if (fromAngle === undefined) return straightShape(edge.from, edge.to);
  const toId = edge.kind === "loop" ? RING_START : edge.to;
  const toAngle = ANGLE_BY_NODE.get(toId);
  if (toAngle === undefined) return straightShape(edge.from, toId);
  return arcShape(edge.from, fromAngle, toId, toAngle);
}

/** 出口：从节点沿半径方向向外伸出一小段，末端一个菱形端子（不再是节点）。 */
function exitStubOf(node: AgentNodeId): { start: Point; end: Point } {
  const center = centerOf(node);
  const angle = ANGLE_BY_NODE.get(node);
  const outward =
    angle === undefined
      ? { x: -1, y: 0 }
      : normalize({
          x: Math.sin((angle * Math.PI) / 180),
          y: -Math.cos((angle * Math.PI) / 180),
        });
  const start = boxEdge(center, outward);
  return {
    start,
    end: { x: start.x + outward.x * 52, y: start.y + outward.y * 52 },
  };
}

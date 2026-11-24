import type { I18nPath } from "@/i18n";
import type {
  CanvasBox,
  CanvasEdgeView,
  CanvasLabelView,
} from "../canvas/types";
import type { OrchCardContext, WorkflowEdge, WorkflowGroup } from "./types";

/**
 * 编排画布的非卡片对象（I69d §5）：边、条件标签、分组标题标签。
 * 三者都不产生 `CanvasCardView`，不进选中序列；几何由两端盒子推导，不存模板。
 */

const CONDITION_LABEL_SIZE = { w: 44, h: 18 };
const GROUP_LABEL_PADDING = 8;

const CONDITION_KEYS: Record<WorkflowEdge["condition"], I18nPath | null> = {
  always: null,
  onSuccess: "orch.edge.cond.onSuccess",
  onFailure: "orch.edge.cond.onFailure",
};

function centerOf(box: CanvasBox): { x: number; y: number } {
  return { x: box.x + box.w / 2, y: box.y + box.h / 2 };
}

/**
 * 边（I69d §5.1）：端点节点级直映射；路由只取 `horizontal` / `vertical`——
 * 模板是 DAG，无回边，不产出 `outer-track`。编辑态 `phase` 恒 `idle`。
 */
export function orchEdgeOf(
  edge: WorkflowEdge,
  from: CanvasBox,
  to: CanvasBox,
): CanvasEdgeView {
  const a = centerOf(from);
  const b = centerOf(to);
  const route =
    Math.abs(b.x - a.x) >= Math.abs(b.y - a.y) ? "horizontal" : "vertical";
  return { id: edge.id, from: edge.from, to: edge.to, route, phase: "idle" };
}

/**
 * 条件标签（I69d §5.2）：`always` 不生成；其余在两端盒子中心连线的中点放一个只读标签。
 * 标签不参与几何预算，绝对定位覆盖在连线之上。
 */
export function orchEdgeLabelOf(
  edge: WorkflowEdge,
  from: CanvasBox,
  to: CanvasBox,
  t: OrchCardContext["t"],
): CanvasLabelView | null {
  const key = CONDITION_KEYS[edge.condition];
  if (key === null) return null;
  const a = centerOf(from);
  const b = centerOf(to);
  return {
    id: `cond-${edge.id}`,
    box: {
      x: (a.x + b.x) / 2 - CONDITION_LABEL_SIZE.w / 2,
      y: (a.y + b.y) / 2 - CONDITION_LABEL_SIZE.h / 2,
      w: CONDITION_LABEL_SIZE.w,
      h: CONDITION_LABEL_SIZE.h,
    },
    text: t(key),
    tone: "muted",
  };
}

/**
 * 分组标题标签（I69d §5.3）：`box` = 成员包围盒外扩 8px，`text` = 组标题。
 * 空分组不画（保存期已拒，渲染层不补建）；虚线框走 INC3 可选字段，默认关闭。
 */
export function orchGroupLabelOf(
  group: WorkflowGroup,
  boxes: readonly CanvasBox[],
  options?: { frame?: boolean },
): CanvasLabelView | null {
  if (boxes.length === 0) return null;

  const left = Math.min(...boxes.map((box) => box.x)) - GROUP_LABEL_PADDING;
  const top = Math.min(...boxes.map((box) => box.y)) - GROUP_LABEL_PADDING;
  const right =
    Math.max(...boxes.map((box) => box.x + box.w)) + GROUP_LABEL_PADDING;
  const bottom =
    Math.max(...boxes.map((box) => box.y + box.h)) + GROUP_LABEL_PADDING;

  const label: CanvasLabelView = {
    id: `group-${group.id}`,
    box: { x: left, y: top, w: right - left, h: bottom - top },
    text: group.title,
    tone: "muted",
  };
  if (options?.frame === true) label.frame = true;
  return label;
}

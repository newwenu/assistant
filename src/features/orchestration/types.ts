import type { CanvasBox } from "../canvas/types";
import type { I18nPath } from "@/i18n";

/**
 * 编排层的只读视图类型（I69d §1.3）。
 * 与模板 schema 同构的 camelCase 形态；R4 契约落地后换成生成的 wire 类型，
 * 本文件只保留「构造器入参」这一层，不承载本地 UI 状态。
 */

export type NodeKind = "turn" | "hitl";

export type EdgeCondition = "always" | "onSuccess" | "onFailure";

export interface TurnNodeParams {
  content: string;
  modeId: string | null;
  providerId: string | null;
  modelId: string | null;
}

/** `kind` 保持 `string`：闭集外的值必须能进入降级路径而不崩（I69d §3.3 / OC2）。 */
export interface WorkflowNode {
  id: string;
  kind: string;
  title: string;
  params: TurnNodeParams | null;
  groupId: string | null;
}

export interface WorkflowEdge {
  id: string;
  from: string;
  to: string;
  condition: EdgeCondition;
}

export interface WorkflowGroup {
  id: string;
  title: string;
  nodeIds: readonly string[];
}

/** `NodeRunState`（7 值，I69a §8.2）；与 L1 的 `NodeState` 各自独立，不互借（I69b K5）。 */
export type NodeRunState =
  | "pending"
  | "running"
  | "waiting"
  | "finished"
  | "failed"
  | "skipped"
  | "cancelled";

/** 运行态叠加只带视图量：事件 `at` 不进（I69d §2.3）。 */
export interface NodeRunOverlay {
  state: NodeRunState;
  turnId: string | null;
  messageId: string | null;
  reason: string | null;
  note: string | null;
  startedAt: number | null;
}

export interface OrchCardContext {
  box: CanvasBox;
  /** 注入翻译函数：构造器内不读全局 i18n，同输入必得同输出（I69d §2.3）。 */
  t: (key: I18nPath) => string;
  /** 只在运行态算时长；不传即不出 `measure`（I69d §3.1）。 */
  nowMs?: number;
}

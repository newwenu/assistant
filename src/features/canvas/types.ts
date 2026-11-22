export interface Point {
  x: number;
  y: number;
}

export interface CanvasBox {
  x: number;
  y: number;
  w: number;
  h: number;
}

export type CardTone =
  "default" | "active" | "success" | "warning" | "danger" | "muted";

export type BadgeTone = "neutral" | "positive" | "warning" | "negative";

export type EdgePhase = "untaken" | "idle" | "passed" | "active";

export type ExitTone = "failed" | "cancelled" | "stopped";

export interface CanvasBadgeView {
  text: string;
  tone?: BadgeTone;
}

export interface CanvasCardView {
  id: string;
  box: CanvasBox;
  title: string;
  subtitle?: string;
  badges?: readonly CanvasBadgeView[];
  tone?: CardTone;
  exit?: ExitTone | null;
}

export type EdgeRoute = "horizontal" | "vertical" | "outer-track";

export interface CanvasEdgeView {
  id: string;
  from: string;
  to: string;
  route: EdgeRoute;
  phase: EdgePhase;
}

export interface CanvasLabelView {
  id: string;
  box: CanvasBox;
  text: string;
  tone?: "muted" | "default";
}

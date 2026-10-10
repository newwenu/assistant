import type { VisemeShape } from "../../core/resolve";

type VisemeId = "REST" | "A" | "E" | "I" | "O" | "U" | "M" | "F";

export const VISEMES: Record<VisemeId, VisemeShape> = {
  REST: { openness: 0, width: 0.3 },
  A: { openness: 0.95, width: 0.6 },
  E: { openness: 0.45, width: 1.0 },
  I: { openness: 0.3, width: 0.85 },
  O: { openness: 0.75, width: 0.2 },
  U: { openness: 0.4, width: 0.05 },
  M: { openness: 0, width: 0.4 },
  F: { openness: 0.15, width: 0.55 },
};

export const REST_SMILE_PATH = "M478,570 Q520,617 562,570 Q520,597 478,570 Z";

const VISEME_ALIAS: Record<string, VisemeId> = {
  sil: "REST",
  rest: "REST",
  aa: "A",
  a: "A",
  e: "E",
  ih: "I",
  i: "I",
  y: "I",
  oh: "O",
  o: "O",
  ou: "U",
  u: "U",
  w: "U",
  m: "M",
  pp: "M",
  b: "M",
  f: "F",
  ff: "F",
  v: "F",
};

export function resolveSvgViseme(code: string): VisemeShape {
  const id = VISEME_ALIAS[code.toLowerCase()] ?? "REST";
  return VISEMES[id];
}

const CX = 520;
const CY = 590;

export function visemePath(shape: VisemeShape): string {
  if (shape.openness < 0.05) return REST_SMILE_PATH;
  const halfW = 22 + shape.width * 28;
  const halfH = 4 + shape.openness * 28;
  const left = CX - halfW;
  const right = CX + halfW;
  const top = CY - halfH;
  const bot = CY + halfH;
  return `M${left},${CY} Q${CX},${top} ${right},${CY} Q${CX},${bot} ${left},${CY} Z`;
}

import { orbService } from "../services/orbService";
import { getCurrentWindow, PhysicalPosition } from "@tauri-apps/api/window";

export interface LogicalPos {
  x: number;
  y: number;
}

export interface LogicalSize {
  width: number;
  height: number;
}

export interface LogicalRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

const dpr = () => window.devicePixelRatio || 1;

export function physicalToLogical(px: number, py: number): LogicalPos {
  const s = dpr();
  return { x: px / s, y: py / s };
}

export function physicalSizeToLogical(pw: number, ph: number): LogicalSize {
  const s = dpr();
  return { width: pw / s, height: ph / s };
}

export function logicalToPhysical(lx: number, ly: number): PhysicalPosition {
  const s = dpr();
  return new PhysicalPosition(Math.round(lx * s), Math.round(ly * s));
}

export async function getMouseLogicalPos(): Promise<LogicalPos> {
  const pos = await orbService.getMousePosition();
  return physicalToLogical(pos.client_x, pos.client_y);
}

export async function getWindowLogicalRect(): Promise<LogicalRect> {
  const win = getCurrentWindow();
  const [pos, size] = await Promise.all([win.outerPosition(), win.outerSize()]);
  const lp = physicalToLogical(pos.x, pos.y);
  const ls = physicalSizeToLogical(size.width, size.height);
  return { x: lp.x, y: lp.y, width: ls.width, height: ls.height };
}

export function isPointInRect(
  px: number,
  py: number,
  rect: LogicalRect,
): boolean {
  return (
    px >= rect.x &&
    px < rect.x + rect.width &&
    py >= rect.y &&
    py < rect.y + rect.height
  );
}

export function isPointInCircle(
  px: number,
  py: number,
  cx: number,
  cy: number,
  r: number,
): boolean {
  const dx = px - cx;
  const dy = py - cy;
  return dx * dx + dy * dy <= r * r;
}

import { describe, expect, it } from "vitest";
import { CANVAS_METRICS } from "@/features/canvas/layout";
import { readRepoFile } from "@/test-utils/paths";

const canvasCss = readRepoFile("src/features/canvas/canvas.module.css");
const globalCss = readRepoFile("src/assets/styles/global.css");

function cssPx(name: string): number {
  const match = globalCss.match(new RegExp(`--${name}:\\s*(\\d+)px`));
  if (match === null) throw new Error(`missing css variable --${name}`);
  return Number(match[1]);
}

describe("canvas encoding", () => {
  it("keeps CANVAS_METRICS in sync with the --canvas-* variables (K23)", () => {
    expect(cssPx("canvas-cell-w")).toBe(CANVAS_METRICS.cellW);
    expect(cssPx("canvas-cell-h")).toBe(CANVAS_METRICS.cellH);
    expect(cssPx("canvas-gap")).toBe(CANVAS_METRICS.gap);
    expect(cssPx("canvas-loop-gutter")).toBe(CANVAS_METRICS.loopGutter);
  });

  it("encodes the four phases on data-phase only (K9)", () => {
    for (const phase of ["untaken", "idle", "passed", "active"]) {
      expect(canvasCss).toContain(`.edgeLine[data-phase="${phase}"]`);
    }
    expect(canvasCss).toContain("repeating-linear-gradient");
    expect(canvasCss).toContain("min-height: 2px");
    expect(canvasCss).toContain("min-width: 2px");
  });

  it("animates the active phase only and keeps reduced motion (K10)", () => {
    expect(canvasCss).toContain("@keyframes canvas-edge-flow");
    expect(canvasCss).toContain("@keyframes canvas-edge-flow-vertical");
    expect(canvasCss).toContain("@media (prefers-reduced-motion: reduce)");
    expect(canvasCss).toContain("animation: none");
  });

  it("uses theme variables without fallbacks or hardcoded colours (K11)", () => {
    expect(canvasCss).not.toMatch(/var\(--[^)]*,/);
    expect(canvasCss).not.toMatch(/#[0-9a-fA-F]{3,8}\b/);
    expect(canvasCss).not.toMatch(/rgba?\(/);
    expect(canvasCss).toContain("var(--ant-color-primary)");
  });
});

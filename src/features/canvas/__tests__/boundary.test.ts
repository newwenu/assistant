import fs from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { repoPath } from "@/test-utils/paths";

const CANVAS_DIR = repoPath("src/features/canvas");
const DEMO_DIR = path.join(CANVAS_DIR, "demo");

function listFiles(dir: string): string[] {
  if (!fs.existsSync(dir)) return [];
  return fs.readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) return listFiles(full);
    if (!/\.(ts|tsx|css)$/.test(entry.name)) return [];
    return [full];
  });
}

function readAll(files: string[]): string {
  return files.map((file) => fs.readFileSync(file, "utf8")).join("\n");
}

function readModule(name: string): string {
  return fs.readFileSync(path.join(CANVAS_DIR, name), "utf8");
}

const facilityFiles = listFiles(CANVAS_DIR).filter(
  (file) => !file.startsWith(DEMO_DIR) && !file.includes("__tests__"),
);

const PURE_MODULES = ["layout.ts", "geometry.ts", "viewport.ts", "types.ts"];

const RENDER_COMPONENTS = [
  "CanvasCard.tsx",
  "CanvasEdge.tsx",
  "CanvasLabel.tsx",
  "CanvasLegend.tsx",
];

const MEASURE_TOKENS = [
  "getBoundingClientRect",
  "offsetWidth",
  "offsetHeight",
  "ResizeObserver",
];

describe("canvas facility boundary", () => {
  it("never imports or names the observation domain", () => {
    const source = readAll(facilityFiles);

    expect(source).not.toContain("@/stores/agent");
    expect(source).not.toContain("@/features/observation");
    expect(source).not.toContain("AgentNodeId");
    expect(source).not.toContain("GraphView");
    expect(source).not.toContain("TurnObservationStatus");
  });

  it("keeps every pure module free of measurement", () => {
    for (const name of PURE_MODULES) {
      const source = readModule(name);
      for (const token of MEASURE_TOKENS) {
        expect(source, `${name} must not contain ${token}`).not.toContain(
          token,
        );
      }
    }
  });

  it("keeps the render path free of measurement", () => {
    for (const name of RENDER_COMPONENTS) {
      const source = readModule(name);
      for (const token of MEASURE_TOKENS) {
        expect(source, `${name} must not contain ${token}`).not.toContain(
          token,
        );
      }
    }
  });

  it("reads the surface rect only inside CanvasViewport event handlers", () => {
    const source = readModule("CanvasViewport.tsx");

    expect(source).not.toContain("offsetWidth");
    expect(source).not.toContain("offsetHeight");
    expect(source).not.toContain("ResizeObserver");
    expect(source.match(/getBoundingClientRect/g)?.length ?? 0).toBe(1);
  });

  it("keeps the world layer as the single transform point", () => {
    for (const name of RENDER_COMPONENTS) {
      const source = readModule(name);
      expect(source, `${name} must not transform itself`).not.toContain(
        "translate",
      );
      expect(source, `${name} must not scale itself`).not.toContain("scale(");
    }
  });

  it("keeps demo content free of observation identifiers", () => {
    const source = readAll(listFiles(DEMO_DIR));

    expect(source).not.toContain("AgentNodeId");
    expect(source).not.toContain("GraphView");
    expect(source).not.toContain("TurnObservationStatus");
    expect(source).not.toContain("@/stores/agent");
    expect(source).not.toContain("@/features/observation");
  });
});

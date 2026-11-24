import fs from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { readRepoFile, repoPath } from "@/test-utils/paths";

const ORCH_DIR = repoPath("src/features/orchestration");
const CANVAS_CARD = readRepoFile("src/features/canvas/CanvasCard.tsx");
const CANVAS_LABEL = readRepoFile("src/features/canvas/CanvasLabel.tsx");
const CANVAS_CSS = readRepoFile("src/features/canvas/canvas.module.css");
const OBSERVATION_CONTENT = readRepoFile(
  "src/features/observation/canvasContent.ts",
);
const ORCH_CSS = readRepoFile(
  "src/features/orchestration/orchestration.module.css",
);
const ORCH_CARD_KINDS = readRepoFile("src/features/orchestration/cardKinds.ts");

const FORBIDDEN_IMPORTS = [
  "features/observation",
  "presets/cardKinds",
  "presets/builtinAgentPreset",
  "presets/phaseAggregate",
  "presets/stagePhases",
];

const FORBIDDEN_CALLS = [
  "cardViewOf",
  "exampleCardOf",
  "paletteItemsOf",
  "canvasContentOf",
  "stageTrailOf",
];

const COLOR_LITERAL = /#[0-9a-fA-F]{3,8}\b|rgba?\(|hsla?\(/;

/** 递归列出目录下的源码文件（不含 `__tests__`）。 */
function sourceFilesOf(dir: string): string[] {
  const out: string[] = [];
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    if (entry.name === "__tests__") continue;
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      out.push(...sourceFilesOf(full));
      continue;
    }
    if (/\.(ts|tsx|css)$/.test(entry.name)) out.push(full);
  }
  return out;
}

/** 去掉块注释与行注释，避免注释里的字面量被当成代码。 */
function codeOf(source: string): string {
  return source
    .replace(/\/\*[\s\S]*?\*\//g, "")
    .replace(/^\s*\/\/.*$/gm, "")
    .replace(/\s\/\/\s.*$/gm, "");
}

function importsOf(source: string): string[] {
  return [...source.matchAll(/from\s+"([^"]+)"/g)].map((m) => m[1]);
}

const ORCH_SOURCES = sourceFilesOf(ORCH_DIR).map((file) => ({
  name: path.relative(ORCH_DIR, file),
  code: codeOf(fs.readFileSync(file, "utf8")),
}));

describe("OC1 — 编排侧不依赖观测侧示例构造器", () => {
  it("does not import the observation content layer or the example tables", () => {
    for (const { name, code } of ORCH_SOURCES) {
      for (const spec of importsOf(code)) {
        expect(
          FORBIDDEN_IMPORTS.some((bad) => spec.includes(bad)),
          `${name} imports ${spec}`,
        ).toBe(false);
      }
    }
  });

  it("never calls the example constructors anywhere in orchestration", () => {
    for (const { name, code } of ORCH_SOURCES) {
      for (const call of FORBIDDEN_CALLS) {
        expect(code.includes(call), `${name} calls ${call}`).toBe(false);
      }
    }
  });

  it("keeps the example table free of orchestration imports", () => {
    const exampleTable = readRepoFile(
      "src/features/canvas/presets/cardKinds.ts",
    );
    for (const spec of importsOf(codeOf(exampleTable))) {
      expect(spec.includes("orchestration"), spec).toBe(false);
    }
  });
});

describe("OC12 — 三处增量默认关闭，观测侧零回归", () => {
  it("keeps the observation projection free of the new shapes", () => {
    const code = codeOf(OBSERVATION_CONTENT);
    expect(code.includes('"detail"')).toBe(false);
    expect(code.includes("frame")).toBe(false);
  });

  it("renders the run band only behind the status guard", () => {
    expect(CANVAS_CARD).toMatch(
      /card\.status !== undefined && \(\s*<span\s+className={styles\.cardRunBand}/,
    );
    expect(CANVAS_CARD).toMatch(/data-tone={card\.tone \?\? "default"}/);
  });

  it("renders the dashed frame only when the field is true", () => {
    expect(CANVAS_LABEL).toMatch(/data-frame={label\.frame === true}/);
  });

  it("keeps all three increments in the stylesheet", () => {
    expect(CANVAS_CSS).toMatch(/\.cardRunBand\s*\{/);
    expect(CANVAS_CSS).toMatch(/\.cardDetail\s*\{/);
    expect(CANVAS_CSS).toMatch(/\.label\[data-frame="true"\]\s*\{/);
  });
});

describe("OC10 — 卡片内零交互控件", () => {
  it("keeps the card root as the only button and adds no form control", () => {
    expect(CANVAS_CARD.match(/<button/g)).toHaveLength(1);
    for (const control of ["<input", "<select", "<textarea"]) {
      expect(CANVAS_CARD.includes(control), control).toBe(false);
    }
  });
});

describe("OC14 — 颜色只来自 CSS 变量", () => {
  it("writes no color literal in orchestration styles", () => {
    expect(COLOR_LITERAL.test(ORCH_CSS)).toBe(false);
    expect(COLOR_LITERAL.test(CANVAS_CSS)).toBe(false);
  });

  it("passes tone as a token, never as a raw color", () => {
    expect(ORCH_CARD_KINDS).not.toMatch(/tone:\s*"[#rgb]/);
    expect(CANVAS_CARD).not.toMatch(/color:\s*"[#rgb]/);
  });
});

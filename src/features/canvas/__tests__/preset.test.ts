import { describe, expect, it } from "vitest";
import enUS from "@/i18n/locales/en-US.json";
import zhCN from "@/i18n/locales/zh-CN.json";
import { layoutTemplate } from "@/features/canvas/layout";
import { CARD_METRICS } from "@/features/canvas/presets/metrics";
import { CARD_KINDS, CARD_KIND_IDS } from "@/features/canvas/presets/cardKinds";
import {
  BUILTIN_AGENT_LAYOUT,
  BUILTIN_AGENT_PRESET,
} from "@/features/canvas/presets/builtinAgentPreset";
import {
  layoutPreset,
  zoneLabelBoxOf,
} from "@/features/canvas/presets/placement";

const L2_WORDS = ["接线", "参数表单", "执行单元"];

const UI_KEYS = [
  "canvas.preset.badge_builtin",
  "canvas.preset.badge_example",
  "canvas.preset.readonly_hint",
  "canvas.palette.title",
  "canvas.card.badge_example",
  "canvas.card.category.execution",
  "canvas.card.category.container",
  "canvas.card.stages",
  "canvas.phase.settle",
  "canvas.phase.decide",
  "canvas.phase.commit",
  "canvas.phase.tools",
  "canvas.phase.wrap",
  "canvas.card.reference",
  "canvas.card.body.output",
  "canvas.card.body.messages",
  "canvas.card.body.question",
  "canvas.card.body.subgraph",
];

function textAtPath(locale: object, path: string): string | undefined {
  let cursor: unknown = locale;
  for (const key of path.split(".")) {
    if (typeof cursor !== "object" || cursor === null) return undefined;
    cursor = (cursor as Record<string, unknown>)[key];
  }
  return typeof cursor === "string" ? cursor : undefined;
}

function rowOfZone(zoneId: string): number {
  const zone = BUILTIN_AGENT_PRESET.zones.find((item) => item.id === zoneId);
  if (zone === undefined) throw new Error(`unknown zone ${zoneId}`);
  return zone.row;
}

describe("built-in agent preset", () => {
  it("declares exactly one card per example kind", () => {
    const kinds = BUILTIN_AGENT_PRESET.cards.map((card) => card.kind);

    expect([...kinds].sort()).toEqual([...CARD_KIND_IDS].sort());
    expect(new Set(kinds).size).toBe(CARD_KIND_IDS.length);
    for (const kind of kinds) {
      expect(CARD_KINDS[kind]).toBeDefined();
    }
  });

  it("pins every card to a declared zone row", () => {
    for (const card of BUILTIN_AGENT_PRESET.cards) {
      expect(rowOfZone(card.zone)).toBeGreaterThanOrEqual(0);
      expect(card.column).toBeGreaterThanOrEqual(0);
    }
  });

  it("declares example connectors instead of template edges", () => {
    expect(BUILTIN_AGENT_PRESET.connectorPolicy).toEqual({
      source: "preset-example",
      editable: true,
    });

    const connectors = BUILTIN_AGENT_PRESET.connectors;
    expect(connectors).toHaveLength(4);
    expect(
      connectors.filter((connector) => connector.route === "horizontal"),
    ).toHaveLength(3);
    expect(
      connectors.filter((connector) => connector.route === "vertical"),
    ).toHaveLength(1);
    for (const connector of connectors) {
      expect(CARD_KINDS[connector.from]).toBeDefined();
      expect(CARD_KINDS[connector.to]).toBeDefined();
      expect(connector.from).not.toBe(connector.to);
    }
  });

  it("is a pure function of the preset", () => {
    const first = layoutPreset(BUILTIN_AGENT_PRESET);
    const second = layoutPreset(BUILTIN_AGENT_PRESET);

    expect(Array.from(second.boxes.entries())).toEqual(
      Array.from(first.boxes.entries()),
    );
    expect(second.width).toBe(first.width);
    expect(second.height).toBe(first.height);
  });

  it("places the five boxes without overlap", () => {
    const boxes = [...BUILTIN_AGENT_LAYOUT.boxes.values()];

    expect(boxes.length).toBe(CARD_KIND_IDS.length);
    for (let left = 0; left < boxes.length; left += 1) {
      for (let right = left + 1; right < boxes.length; right += 1) {
        const a = boxes[left];
        const b = boxes[right];
        const overlap =
          a.x < b.x + b.w &&
          b.x < a.x + a.w &&
          a.y < b.y + b.h &&
          b.y < a.y + a.h;

        expect(overlap).toBe(false);
      }
    }
  });

  it("delegates geometry to layoutTemplate with card metrics", () => {
    const manual = layoutTemplate(
      BUILTIN_AGENT_PRESET.cards.map((card) => ({
        id: card.kind,
        row: rowOfZone(card.zone),
        column: card.column,
      })),
      CARD_METRICS,
    );

    expect(Array.from(BUILTIN_AGENT_LAYOUT.boxes.entries())).toEqual(
      Array.from(manual.boxes.entries()),
    );
    expect(BUILTIN_AGENT_LAYOUT.width).toBe(1296);
    expect(BUILTIN_AGENT_LAYOUT.height).toBe(784);
  });

  it("labels the branch lane in the free columns", () => {
    const branchLane = BUILTIN_AGENT_PRESET.zones.find(
      (zone) => zone.id === "branch-lane",
    );
    const mainLane = BUILTIN_AGENT_PRESET.zones.find(
      (zone) => zone.id === "main-lane",
    );
    if (branchLane === undefined || mainLane === undefined) {
      throw new Error("built-in preset must declare both zones");
    }

    expect(mainLane.labelColumns).toBeNull();
    expect(zoneLabelBoxOf(mainLane)).toBeNull();

    expect(zoneLabelBoxOf(branchLane)).toEqual({
      x: 0,
      y: 424,
      w: 632,
      h: 360,
    });

    const cardColumns = new Set(
      BUILTIN_AGENT_PRESET.cards
        .filter((card) => card.zone === "branch-lane")
        .map((card) => card.column),
    );
    for (const column of branchLane.labelColumns ?? []) {
      expect(cardColumns.has(column)).toBe(false);
    }
  });

  it("keeps preset identifiers resolvable in both locales", () => {
    const keys = [
      BUILTIN_AGENT_PRESET.titleKey,
      BUILTIN_AGENT_PRESET.descriptionKey,
      ...BUILTIN_AGENT_PRESET.zones.flatMap((zone) =>
        zone.labelKey === null ? [] : [zone.labelKey],
      ),
      ...CARD_KIND_IDS.flatMap((kind) => {
        const spec = CARD_KINDS[kind];
        return [
          spec.nameKey,
          spec.summaryKey,
          ...(spec.formKey === null ? [] : [spec.formKey]),
          ...spec.params.map((param) => param.labelKey),
        ];
      }),
      ...UI_KEYS,
    ];

    for (const key of keys) {
      expect(textAtPath(zhCN, key), `zh-CN ${key}`).toBeTruthy();
      expect(textAtPath(enUS, key), `en-US ${key}`).toBeTruthy();
    }
  });

  it("keeps preset copy free of L2 vocabulary", () => {
    const keys = [
      BUILTIN_AGENT_PRESET.titleKey,
      BUILTIN_AGENT_PRESET.descriptionKey,
      ...BUILTIN_AGENT_PRESET.zones.flatMap((zone) =>
        zone.labelKey === null ? [] : [zone.labelKey],
      ),
    ];

    for (const locale of [zhCN, enUS]) {
      for (const key of keys) {
        const text = textAtPath(locale, key) ?? "";
        for (const word of L2_WORDS) {
          expect(text, `${key} must not contain ${word}`).not.toContain(word);
        }
      }
    }
  });
});

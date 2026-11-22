/**
 * 内置 Agent 画布预设（I68 §4）：5 张示例卡片 + 两条区带 + 4 条示例连线。
 * 只声明区带与列位，不存坐标（A12）；连线集合由预设显式声明，不读模板边集（A5）。
 */
import { layoutPreset } from "./placement";
import type {
  CanvasPreset,
  CanvasZone,
  PresetCard,
  PresetConnector,
  PresetConnectorPolicy,
} from "./types";

const ZONES: readonly CanvasZone[] = Object.freeze([
  Object.freeze({
    id: "main-lane",
    row: 0,
    labelKey: null,
    labelColumns: null,
  }),
  Object.freeze({
    id: "branch-lane",
    row: 1,
    labelKey: "canvas.preset.zone.branch_lane",
    labelColumns: Object.freeze([0, 1] as const),
  }),
] satisfies readonly CanvasZone[]);

const CARDS: readonly PresetCard[] = Object.freeze([
  Object.freeze({
    kind: "execution-conversation",
    zone: "main-lane",
    column: 0,
  }),
  Object.freeze({ kind: "execution-turn", zone: "main-lane", column: 1 }),
  Object.freeze({ kind: "execution-tool", zone: "main-lane", column: 2 }),
  Object.freeze({ kind: "execution-hitl", zone: "branch-lane", column: 2 }),
  Object.freeze({
    kind: "container-conversation",
    zone: "branch-lane",
    column: 3,
  }),
] satisfies readonly PresetCard[]);

const CONNECTORS: readonly PresetConnector[] = Object.freeze([
  Object.freeze({
    from: "execution-conversation",
    to: "execution-turn",
    route: "horizontal",
  }),
  Object.freeze({
    from: "execution-turn",
    to: "execution-tool",
    route: "horizontal",
  }),
  Object.freeze({
    from: "execution-tool",
    to: "execution-hitl",
    route: "vertical",
  }),
  Object.freeze({
    from: "execution-hitl",
    to: "container-conversation",
    route: "horizontal",
  }),
] satisfies readonly PresetConnector[]);

const CONNECTOR_POLICY: PresetConnectorPolicy = Object.freeze({
  source: "preset-example",
  editable: true,
});

export const BUILTIN_AGENT_PRESET: CanvasPreset = Object.freeze({
  id: "builtin-agent-turn",
  builtin: true,
  titleKey: "canvas.preset.builtin_agent.name",
  descriptionKey: "canvas.preset.builtin_agent.description",
  zones: ZONES,
  cards: CARDS,
  connectors: CONNECTORS,
  connectorPolicy: CONNECTOR_POLICY,
  stages: Object.freeze({ source: "template-nodes" }),
});

export const BUILTIN_AGENT_LAYOUT = layoutPreset(BUILTIN_AGENT_PRESET);

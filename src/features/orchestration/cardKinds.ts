import type { I18nPath } from "@/i18n";
import type { CanvasPaletteItem } from "../canvas/CanvasPalette";
import type {
  CanvasCardBodyView,
  CanvasCardParamView,
  CanvasCardPortView,
  CanvasCardView,
  CardTone,
} from "../canvas/types";
import type {
  NodeRunOverlay,
  NodeRunState,
  OrchCardContext,
  TurnNodeParams,
  WorkflowNode,
} from "./types";

/**
 * 编排卡片表（I69d §2.3）：与示例表 `presets/cardKinds.ts` 分列，不互相 import。
 * 表内只有类别身份与默认文案键，不含示例值、参数占位与阶段轨道。
 */
export interface OrchCardKind {
  key: "turn" | "hitl";
  kindBadgeKey: I18nPath;
  defaultTitleKey: I18nPath;
  editTone: CardTone;
  hasParams: boolean;
}

const TURN_KIND: OrchCardKind = {
  key: "turn",
  kindBadgeKey: "orch.card.kind.turn",
  defaultTitleKey: "orch.card.name.turn",
  editTone: "default",
  hasParams: true,
};

const HITL_KIND: OrchCardKind = {
  key: "hitl",
  kindBadgeKey: "orch.card.kind.hitl",
  defaultTitleKey: "orch.card.name.hitl",
  editTone: "warning",
  hasParams: false,
};

/** 恰好两项，顺序固定（I69d §2.3）。降级形态不是类别，不进本表。 */
export const ORCH_CARD_KINDS: readonly OrchCardKind[] = [TURN_KIND, HITL_KIND];

export function isOrchCardKind(kind: string): kind is OrchCardKind["key"] {
  return ORCH_CARD_KINDS.some((item) => item.key === kind);
}

/** 端口只画圆点，标签必填空串（I69d §5.4 / OC7）；降级卡不画端口。 */
const ORCH_PORTS: readonly CanvasCardPortView[] = [
  { side: "left", typeLabel: "" },
  { side: "right", typeLabel: "" },
];

/** 运行态色带与药丸同源同值（I69d §4.2 / INC2）。 */
const RUN_TONES: Record<NodeRunState, CardTone> = {
  pending: "default",
  running: "active",
  waiting: "warning",
  finished: "success",
  failed: "danger",
  skipped: "muted",
  cancelled: "muted",
};

const RUN_STATE_KEYS: Record<NodeRunState, I18nPath> = {
  pending: "orch.node.state.pending",
  running: "orch.node.state.running",
  waiting: "orch.node.state.waiting",
  finished: "orch.node.state.finished",
  failed: "orch.node.state.failed",
  skipped: "orch.node.state.skipped",
  cancelled: "orch.node.state.cancelled",
};

const DASH = "—";

function present(value: string | null | undefined): value is string {
  return value !== null && value !== undefined && value.trim() !== "";
}

function paramsOf(
  params: TurnNodeParams,
  t: OrchCardContext["t"],
): readonly CanvasCardParamView[] {
  const mode = present(params.modeId)
    ? params.modeId
    : t("orch.card.value.inherited");
  const provider = present(params.providerId) ? params.providerId : null;
  const model = present(params.modelId) ? params.modelId : null;
  const modelText =
    provider !== null && model !== null
      ? `${provider} / ${model}`
      : (provider ?? model ?? t("orch.card.value.inherited"));

  return [
    {
      label: t("orch.card.param.content"),
      value: present(params.content) ? params.content : DASH,
    },
    { label: t("orch.card.param.mode"), value: mode },
    { label: t("orch.card.param.model"), value: modelText },
  ];
}

function detailOf(
  key: OrchCardKind["key"],
  run: NodeRunOverlay | null,
  t: OrchCardContext["t"],
): CanvasCardBodyView {
  const label = key === "hitl" ? t("orch.card.hitl.section") : "";

  if (run === null) {
    return {
      kind: "detail",
      label,
      rows: [],
      emptyHint: t("orch.card.empty.idle"),
    };
  }

  const turnRows: readonly { label: string; value: string }[] =
    run.turnId !== null
      ? [{ label: t("orch.card.row.turn"), value: run.turnId }]
      : [];
  const resultRows: readonly { label: string; value: string }[] = [
    {
      label: t("orch.card.row.result"),
      value: present(run.messageId) ? run.messageId : DASH,
    },
  ];
  const reasonRows: readonly { label: string; value: string }[] = [
    {
      label: t("orch.card.row.reason"),
      value: present(run.reason) ? run.reason : t("orch.node.state.failed"),
    },
  ];
  const noteRows: readonly { label: string; value: string }[] = [
    {
      label: t("orch.card.row.note"),
      value: present(run.note) ? run.note : DASH,
    },
  ];

  switch (run.state) {
    case "pending":
      return {
        kind: "detail",
        label,
        rows: [],
        emptyHint: t("orch.card.empty.pending"),
      };
    case "skipped":
      return {
        kind: "detail",
        label,
        rows: [],
        emptyHint: t("orch.card.empty.skipped"),
      };
    case "cancelled":
      return {
        kind: "detail",
        label,
        rows: [],
        emptyHint: t("orch.card.empty.cancelled"),
      };
    case "running":
      return {
        kind: "detail",
        label,
        rows: key === "turn" ? turnRows : noteRows,
      };
    case "waiting":
      return {
        kind: "detail",
        label,
        rows:
          key === "turn"
            ? turnRows
            : [
                ...noteRows,
                {
                  label: t("orch.card.row.decision"),
                  value: t("orch.card.value.decision_pending"),
                },
              ],
      };
    case "finished":
      return { kind: "detail", label, rows: resultRows };
    case "failed":
      return { kind: "detail", label, rows: reasonRows };
  }
}

/** 原语量时长（`1.2s` / `2m 05s`）：只在打开面板时算一次，不做秒级 tick。 */
function elapsedTextOf(ms: number): string {
  const safe = Math.max(0, ms);
  if (safe < 60_000) return `${(safe / 1000).toFixed(1)}s`;
  const minutes = Math.floor(safe / 60_000);
  const seconds = Math.floor((safe % 60_000) / 1000);
  return `${minutes}m ${String(seconds).padStart(2, "0")}s`;
}

/**
 * 编排卡片构造器（I69d §3）：`run === null` 即编辑态，`run !== null` 即运行态。
 * 两态同源——只改附加区、药丸、色带与时长，参数区与几何不动。
 */
export function orchCardOf(
  node: WorkflowNode,
  run: NodeRunOverlay | null,
  ctx: OrchCardContext,
): CanvasCardView {
  if (!isOrchCardKind(node.kind)) return orchFallbackCardOf(node, ctx);

  const spec = node.kind === "turn" ? TURN_KIND : HITL_KIND;
  const title =
    node.title.trim() !== "" ? node.title : ctx.t(spec.defaultTitleKey);

  const card: CanvasCardView = {
    id: node.id,
    box: ctx.box,
    kindBadge: ctx.t(spec.kindBadgeKey),
    title,
    body: detailOf(spec.key, run, ctx.t),
    ports: ORCH_PORTS,
    tone: run === null ? spec.editTone : RUN_TONES[run.state],
  };

  if (spec.hasParams && node.params !== null) {
    card.params = paramsOf(node.params, ctx.t);
  }

  if (run !== null) {
    card.status = ctx.t(RUN_STATE_KEYS[run.state]);
    if (run.startedAt !== null && ctx.nowMs !== undefined) {
      card.measure = elapsedTextOf(ctx.nowMs - run.startedAt);
    }
  }

  return card;
}

/**
 * 未知类别的降级卡（I69d §3.3）：原始 `kind` 原样显示、不画端口与药丸、`tone: muted`。
 * 正常不可达（版本不等即拒加载），只为「模板被外部改动」时不崩溃、不改写模板数据。
 */
export function orchFallbackCardOf(
  node: WorkflowNode,
  ctx: OrchCardContext,
): CanvasCardView {
  return {
    id: node.id,
    box: ctx.box,
    kindBadge: `${node.kind} · ${ctx.t("orch.card.kind.unknown")}`,
    title: node.title.trim() !== "" ? node.title : node.id,
    body: {
      kind: "detail",
      label: "",
      rows: [],
      emptyHint: ctx.t("orch.card.empty.unknown_kind"),
    },
    tone: "muted",
  };
}

/** 放置栏只列 `ORCH_CARD_KINDS` 两项，不生成其余示例类别（I69d §2.3）。 */
export function orchPaletteItemsOf(
  t: OrchCardContext["t"],
): CanvasPaletteItem[] {
  return ORCH_CARD_KINDS.map((item) => ({
    kind: item.key,
    badge: t(item.kindBadgeKey),
    name: t(item.defaultTitleKey),
    summary: t(`orch.card.summary.${item.key}`),
    tone: item.editTone,
  }));
}

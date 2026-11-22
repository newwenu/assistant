import { useCallback, useMemo, useRef, useState } from "react";
import { Alert, Tag, Typography } from "antd";
import { useTranslation } from "react-i18next";
import { CanvasPalette } from "@/features/canvas/CanvasPalette";
import { CanvasPresetBadge } from "@/features/canvas/CanvasPresetBadge";
import {
  CanvasViewport,
  type CanvasPlaceRequest,
} from "@/features/canvas/CanvasViewport";
import { BUILTIN_AGENT_PRESET } from "@/features/canvas/presets/builtinAgentPreset";
import { CARD_KINDS } from "@/features/canvas/presets/cardKinds";
import { CARD_METRICS } from "@/features/canvas/presets/metrics";
import type { CardKindId } from "@/features/canvas/presets/types";
import type { CanvasCardView, Point } from "@/features/canvas/types";
import type { CanvasViewportState } from "@/features/canvas/viewport";
import { useChatStore } from "@/stores/chat/chatStore";
import { observationKey } from "@/stores/agent/observation/types";
import {
  canvasContentOf,
  exampleCardOf,
  paletteItemsOf,
} from "./canvasContent";
import { InspectorPanel } from "./InspectorPanel";
import { useObservationInstances, useProjections } from "./hooks";
import styles from "./observation.module.css";

/** 首次进入时把内容让开左上角的预设标识与左下角图例。 */
const INITIAL_VIEWPORT: CanvasViewportState = { x: 32, y: 64, scale: 1 };

/** 放置栏拖入的预览框尺寸取卡片度量（与真实卡片同尺寸）。 */
const DROP_PREVIEW_SIZE = { w: CARD_METRICS.cellW, h: CARD_METRICS.cellH };

function isCardKindId(kind: string): kind is CardKindId {
  return kind in CARD_KINDS;
}

/**
 * 观测页（I55 §6.2 渲染侧 / I66 §1.3 画布宿主）：只读副本，不参与控制（O6 / G7）。
 *
 * 画布几何来自内置预设（`layoutPreset` → `layoutTemplate`，唯一公式），
 * 卡片文案与色调由内容层 `canvasContentOf` 从观测投影映射；
 * 无实例时同一预设画模板预览（全部 `pending` + `derived`）。
 */
export function ObservationPage() {
  const { t } = useTranslation();
  const instances = useObservationInstances();
  const currentConversationId = useChatStore((s) => s.currentConversationId);
  const [selectedKey, setSelectedKey] = useState<string | null>(null);

  /**
   * 放置栏落卡（I68 §7.10 / A24）：只存页面会话状态——不写模板、不持久化，
   * 刷新或重新进入即消失；卡片 id 用类别 + 自增序号，避免与预设卡片冲突。
   */
  const placedSeq = useRef(0);
  const [placedCards, setPlacedCards] = useState<readonly CanvasCardView[]>([]);
  const [placeRequest, setPlaceRequest] = useState<CanvasPlaceRequest | null>(
    null,
  );

  const placeCard = useCallback(
    (kind: string, point: Point) => {
      if (!isCardKindId(kind)) return;
      const card = exampleCardOf(kind, t);
      placedSeq.current += 1;
      setPlacedCards((prev) => [
        ...prev,
        {
          ...card,
          id: `${kind}#${placedSeq.current}`,
          box: {
            x: point.x - card.box.w / 2,
            y: point.y - card.box.h / 2,
            w: card.box.w,
            h: card.box.h,
          },
        },
      ]);
    },
    [t],
  );

  /** 放置栏回车：只递增令牌，落点由视口按当前视口中心算。 */
  const requestPlace = useCallback((kind: string) => {
    setPlaceRequest((prev) => ({ kind, token: (prev?.token ?? 0) + 1 }));
  }, []);

  const paletteItems = useMemo(() => paletteItemsOf(t), [t]);

  const selected =
    (selectedKey === null
      ? null
      : instances.find(
          (item) =>
            observationKey(item.conversationId, item.turnId) === selectedKey,
        )) ??
    instances[0] ??
    null;

  const projections = useProjections(
    selected?.conversationId ?? null,
    selected?.turnId ?? null,
  );
  const selectedObservationKey =
    selected === null
      ? null
      : observationKey(selected.conversationId, selected.turnId);

  /** 淘汰实例只剩状态摘要（I55 §6.3），画布退回模板预览。 */
  const liveProjections =
    selected !== null && !selected.evicted ? projections : null;
  const live = liveProjections !== null;
  const graph = liveProjections?.graph ?? null;

  const content = useMemo(() => canvasContentOf(graph, t), [graph, t]);

  return (
    <div className={styles.page}>
      <header className={styles.header}>
        <Typography.Title className={styles.headerTitle} level={5}>
          {t("observation.title")}
        </Typography.Title>
        <span className={styles.headerSubtitle}>
          {t("observation.subtitle")}
        </span>
      </header>
      <div className={styles.body} data-empty={instances.length === 0}>
        {instances.length > 0 && (
          <aside className={styles.instancePane}>
            <span className={styles.instancePaneTitle}>
              {t("observation.instances")}
            </span>
            <div className={styles.instanceList}>
              {instances.map((item) => {
                const key = observationKey(item.conversationId, item.turnId);
                return (
                  <button
                    key={key}
                    type="button"
                    className={styles.instanceItem}
                    data-selected={key === selectedObservationKey}
                    onClick={() => setSelectedKey(key)}
                  >
                    <span className={styles.instanceItemHead}>
                      <span className={styles.instanceLabel}>
                        {item.turnIndex === null
                          ? t("observation.turn.unnamed")
                          : t("observation.turn.label", {
                              index: item.turnIndex,
                            })}
                      </span>
                      <Tag
                        color={statusColor(item.status)}
                        style={{ marginInlineEnd: 0 }}
                      >
                        {t(`observation.status.${item.status}`)}
                      </Tag>
                    </span>
                    <span className={styles.instanceMeta}>{item.turnId}</span>
                    <span className={styles.instanceTag}>
                      {item.conversationId === currentConversationId
                        ? t("observation.turn.active")
                        : t("observation.turn.background")}
                    </span>
                  </button>
                );
              })}
            </div>
          </aside>
        )}
        <CanvasPalette
          title={t("canvas.palette.title")}
          hint={t("canvas.palette.hint")}
          placeHint={t("canvas.palette.place_hint")}
          items={paletteItems}
          onPlace={requestPlace}
        />
        <main
          className={styles.mainPane}
          data-inspector={live}
          data-notice={!live}
        >
          <div className={styles.canvasMetaBar}>
            {graph === null ? (
              <span className={styles.canvasMeta}>
                {t("observation.graph.preview")}
              </span>
            ) : (
              <>
                <Tag
                  color={statusColor(graph.status)}
                  style={{ marginInlineEnd: 0 }}
                >
                  {t(`observation.status.${graph.status}`)}
                </Tag>
                <span className={styles.canvasMeta}>
                  {t("observation.graph.iteration", {
                    iteration: graph.iteration,
                  })}
                </span>
                {graph.activeEdge !== null && (
                  <span className={styles.canvasMeta}>
                    {`${t("observation.graph.active_edge")}: `}
                    {t(`observation.node.${graph.activeEdge.from}`)}
                    {" → "}
                    {t(`observation.node.${graph.activeEdge.to}`)}
                    {` (${t(
                      graph.activeEdge.kind === "loop"
                        ? "observation.graph.loop"
                        : "observation.graph.forward",
                    )})`}
                  </span>
                )}
              </>
            )}
            <span className={styles.canvasHint}>
              {t("observation.graph.no_percent")}
            </span>
          </div>
          <div className={styles.canvasHost}>
            <CanvasViewport
              key={selectedObservationKey ?? "template"}
              cards={content.cards}
              edges={content.edges}
              labels={content.labels}
              initialViewport={INITIAL_VIEWPORT}
              placedCards={placedCards}
              onDropCard={placeCard}
              placeRequest={placeRequest}
              dropPreviewSize={DROP_PREVIEW_SIZE}
              header={
                <CanvasPresetBadge
                  name={t(BUILTIN_AGENT_PRESET.titleKey)}
                  description={t(BUILTIN_AGENT_PRESET.descriptionKey)}
                  hint={t("canvas.preset.readonly_hint")}
                />
              }
            />
          </div>
          {liveProjections !== null && (
            <InspectorPanel
              summary={liveProjections.summary}
              timeline={liveProjections.timeline}
            />
          )}
          {live ? null : selected !== null && selected.evicted ? (
            <Alert
              className={styles.notice}
              type="info"
              showIcon
              title={t("observation.turn.evicted")}
              description={t(`observation.status.${selected.status}`)}
            />
          ) : (
            <div className={styles.emptyBar}>
              <span className={styles.emptyTitle}>
                {t("observation.empty.title")}
              </span>
              <span className={styles.canvasHint}>
                {t("observation.empty.description")}
              </span>
            </div>
          )}
        </main>
      </div>
    </div>
  );
}

function statusColor(status: string): string {
  switch (status) {
    case "running":
      return "processing";
    case "completed":
      return "success";
    case "halted_by_budget":
    case "wrap_up":
      return "warning";
    case "failed":
      return "error";
    default:
      return "default";
  }
}

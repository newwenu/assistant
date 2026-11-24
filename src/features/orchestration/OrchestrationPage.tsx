import { useCallback, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { CanvasPalette } from "@/features/canvas/CanvasPalette";
import {
  CanvasViewport,
  type CanvasPlaceRequest,
} from "@/features/canvas/CanvasViewport";
import { CARD_METRICS } from "@/features/canvas/presets/metrics";
import type { CanvasCardView, Point } from "@/features/canvas/types";
import type { CanvasViewportState } from "@/features/canvas/viewport";
import { isOrchCardKind, orchCardOf, orchPaletteItemsOf } from "./cardKinds";
import type { WorkflowNode } from "./types";
import styles from "./orchestration.module.css";

const INITIAL_VIEWPORT: CanvasViewportState = { x: 32, y: 64, scale: 1 };

const DROP_PREVIEW_SIZE = { w: CARD_METRICS.cellW, h: CARD_METRICS.cellH };

const EMPTY_CONTENT = {
  cards: [] as readonly CanvasCardView[],
  edges: [],
  labels: [],
};

/** 放置栏落卡生成的草稿节点：参数留空、标题留空，由构造器给分型默认名。 */
function draftNodeOf(kind: "turn" | "hitl", seq: number): WorkflowNode {
  return {
    id: `${kind}#${seq}`,
    kind,
    title: "",
    params:
      kind === "turn"
        ? { content: "", modeId: null, providerId: null, modelId: null }
        : null,
    groupId: null,
  };
}

/**
 * 编排 Agent 页：画布独占全部可用空间，无实例栏、无固定面板。
 * 放置栏只列 `ORCH_CARD_KINDS` 的两类真实卡片；卡片内容由编排构造器产出，
 * 不调用观测侧的示例构造器（I69d OC1）。模板加载与运行态叠加属 W3（I69c）。
 */
export function OrchestrationPage() {
  const { t } = useTranslation();

  const placedSeq = useRef(0);
  const [placedCards, setPlacedCards] = useState<readonly CanvasCardView[]>([]);
  const [placeRequest, setPlaceRequest] = useState<CanvasPlaceRequest | null>(
    null,
  );

  const placeCard = useCallback(
    (kind: string, point: Point) => {
      if (!isOrchCardKind(kind)) return;
      placedSeq.current += 1;
      const card = orchCardOf(draftNodeOf(kind, placedSeq.current), null, {
        box: {
          x: point.x - CARD_METRICS.cellW / 2,
          y: point.y - CARD_METRICS.cellH / 2,
          w: CARD_METRICS.cellW,
          h: CARD_METRICS.cellH,
        },
        t,
      });
      setPlacedCards((prev) => [...prev, card]);
    },
    [t],
  );

  const requestPlace = useCallback((kind: string) => {
    setPlaceRequest((prev) => ({ kind, token: (prev?.token ?? 0) + 1 }));
  }, []);

  const paletteItems = useMemo(() => orchPaletteItemsOf(t), [t]);

  return (
    <div className={styles.page}>
      <div className={styles.canvasHost}>
        <CanvasViewport
          cards={EMPTY_CONTENT.cards}
          edges={EMPTY_CONTENT.edges}
          labels={EMPTY_CONTENT.labels}
          initialViewport={INITIAL_VIEWPORT}
          placedCards={placedCards}
          onDropCard={placeCard}
          placeRequest={placeRequest}
          dropPreviewSize={DROP_PREVIEW_SIZE}
        />
        <CanvasPalette
          title={t("canvas.palette.title")}
          items={paletteItems}
          onPlace={requestPlace}
        />
      </div>
    </div>
  );
}

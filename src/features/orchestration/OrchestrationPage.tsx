import { useCallback, useMemo, useRef, useState } from "react";
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
import {
  canvasContentOf,
  exampleCardOf,
  paletteItemsOf,
} from "@/features/observation/canvasContent";
import styles from "./orchestration.module.css";

const INITIAL_VIEWPORT: CanvasViewportState = { x: 32, y: 64, scale: 1 };

const DROP_PREVIEW_SIZE = { w: CARD_METRICS.cellW, h: CARD_METRICS.cellH };

function isCardKindId(kind: string): kind is CardKindId {
  return kind in CARD_KINDS;
}

/**
 * 编排 Agent 页：画布独占全部可用空间，无实例栏、无固定面板。
 * 放置栏浮在画布左上角、预设标识浮在画布底部，都不参与布局栅格。
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

  const requestPlace = useCallback((kind: string) => {
    setPlaceRequest((prev) => ({ kind, token: (prev?.token ?? 0) + 1 }));
  }, []);

  const paletteItems = useMemo(() => paletteItemsOf(t), [t]);
  const content = useMemo(() => canvasContentOf(null, t), [t]);

  return (
    <div className={styles.page}>
      <div className={styles.canvasHost}>
        <CanvasViewport
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
        <CanvasPalette
          title={t("canvas.palette.title")}
          items={paletteItems}
          onPlace={requestPlace}
        />
      </div>
    </div>
  );
}

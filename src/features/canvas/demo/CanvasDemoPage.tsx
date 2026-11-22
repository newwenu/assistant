import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { CanvasViewport } from "../CanvasViewport";
import type { CanvasCardView, CanvasLabelView } from "../types";
import {
  DEMO_CARDS,
  DEMO_EDGES,
  DEMO_LABEL_BOX,
  DEMO_LABEL_KEY,
  DEMO_LAYOUT,
} from "./demoContent";
import styles from "./demo.module.css";

/**
 * 示例页（§11.2）：`/canvas-demo` 挂侧栏入口，用于直接查看画布设施。
 * 文案走 `canvas.demo.*`，页面明确标注「示例内容 · 非真实数据」。
 */
export function CanvasDemoPage() {
  const { t } = useTranslation();

  const cards = useMemo<readonly CanvasCardView[]>(
    () =>
      DEMO_CARDS.map((card) => {
        const box = DEMO_LAYOUT.boxes.get(card.id);
        if (box === undefined) {
          throw new Error(`demo card without layout box: ${card.id}`);
        }
        return {
          id: card.id,
          box,
          title: t(card.titleKey),
          subtitle: t(card.subtitleKey),
          exit: card.exit ?? null,
        };
      }),
    [t],
  );

  const labels = useMemo<readonly CanvasLabelView[]>(
    () => [
      {
        id: "demo-lane",
        box: DEMO_LABEL_BOX,
        text: t(DEMO_LABEL_KEY),
        tone: "muted",
      },
    ],
    [t],
  );

  return (
    <div className={styles.page}>
      <CanvasViewport
        cards={cards}
        edges={DEMO_EDGES}
        labels={labels}
        header={<span>{t("canvas.demo.notice")}</span>}
      />
    </div>
  );
}

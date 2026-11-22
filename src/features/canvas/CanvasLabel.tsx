import { memo } from "react";
import type { CanvasLabelView } from "./types";
import styles from "./canvas.module.css";

export interface CanvasLabelProps {
  label: CanvasLabelView;
}

/**
 * 标签（overlays 层，§6.1）：只承载文案与色调，不接点击、不进焦点序列。
 */
function CanvasLabelImpl({ label }: CanvasLabelProps) {
  return (
    <div
      className={styles.label}
      data-tone={label.tone ?? "default"}
      style={{
        left: label.box.x,
        top: label.box.y,
        width: label.box.w,
        height: label.box.h,
      }}
    >
      {label.text}
    </div>
  );
}

export const CanvasLabel = memo(CanvasLabelImpl);

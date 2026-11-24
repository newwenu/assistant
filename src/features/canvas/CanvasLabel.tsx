import { memo } from "react";
import type { CanvasLabelView } from "./types";
import styles from "./canvas.module.css";

export interface CanvasLabelProps {
  label: CanvasLabelView;
}

/**
 * 标签（overlays 层，§6.1）：只承载文案与色调，不接点击、不进焦点序列。
 * `frame` 为真时画虚线框（I69d INC3）；缺省仍为纯文字标签。
 */
function CanvasLabelImpl({ label }: CanvasLabelProps) {
  return (
    <div
      className={styles.label}
      data-tone={label.tone ?? "default"}
      data-frame={label.frame === true}
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

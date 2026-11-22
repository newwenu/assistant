import { memo } from "react";
import { useTranslation } from "react-i18next";
import styles from "./canvas.module.css";

export interface CanvasPresetBadgeProps {
  name: string;
  description: string;
  hint: string;
}

/**
 * 预设标识（I68 §8.1 / A25）：预设名 + 「内置」标记 + 「示例数据」提示 + 编辑说明，
 * 作为视口头信息行的内容。只收已本地化的文本、不引用预设实体，
 * 保持设施层与内容层分离（I66 K24）。
 */
function CanvasPresetBadgeImpl({
  name,
  description,
  hint,
}: CanvasPresetBadgeProps) {
  const { t } = useTranslation();

  return (
    <span className={styles.presetBadge} title={description}>
      <span className={styles.presetName}>{name}</span>
      <span className={styles.badge}>{t("canvas.preset.badge_builtin")}</span>
      <span className={styles.badge}>{t("canvas.preset.badge_example")}</span>
      <span className={styles.presetHint}>{hint}</span>
    </span>
  );
}

export const CanvasPresetBadge = memo(CanvasPresetBadgeImpl);

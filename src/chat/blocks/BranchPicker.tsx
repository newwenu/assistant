import React, { useCallback } from "react";
import { LeftOutlined, RightOutlined, ForkOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import css from "./BranchPicker.module.css";

const MAX_SIBLINGS = 20;
const BRANCH_WARNING_THRESHOLD = 15;

interface BranchPickerProps {
  currentSiblingIndex: number;
  siblingCount: number;
  onSwitch: (direction: "prev" | "next") => void;
  onFork?: () => void;
}

export const BranchPicker: React.FC<BranchPickerProps> = React.memo(
  ({ currentSiblingIndex, siblingCount, onSwitch, onFork }) => {
    const { t } = useTranslation();

    const handlePrev = useCallback(() => onSwitch("prev"), [onSwitch]);
    const handleNext = useCallback(() => onSwitch("next"), [onSwitch]);

    if (siblingCount <= 1) return null;

    const isNearLimit = siblingCount >= BRANCH_WARNING_THRESHOLD;
    const isAtLimit = siblingCount >= MAX_SIBLINGS;

    return (
      <div
        className={[css.root, isNearLimit ? css.warning : ""]
          .filter(Boolean)
          .join(" ")}
      >
        <button
          className={css.arrowBtn}
          disabled={currentSiblingIndex === 0}
          onClick={handlePrev}
          aria-label={t("chat.branch.previous")}
          type="button"
        >
          <LeftOutlined />
        </button>
        <span className={css.label}>
          {currentSiblingIndex + 1} / {siblingCount}
        </span>
        <button
          className={css.arrowBtn}
          disabled={currentSiblingIndex === siblingCount - 1}
          onClick={handleNext}
          aria-label={t("chat.branch.next")}
          type="button"
        >
          <RightOutlined />
        </button>
        {isNearLimit && !isAtLimit && (
          <span className={css.warningText}>
            {t("chat.branch.nearLimitHint", { max: MAX_SIBLINGS })}
          </span>
        )}
        {isAtLimit && onFork && (
          <button
            className={css.forkBtn}
            onClick={onFork}
            aria-label={t("chat.branch.forkAction")}
            type="button"
          >
            <ForkOutlined />
          </button>
        )}
      </div>
    );
  },
);

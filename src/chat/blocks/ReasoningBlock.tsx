import React from "react";
import { useTranslation } from "react-i18next";
import css from "./ReasoningBlock.module.css";

interface ReasoningBlockProps {
  content: string;
  isStreaming?: boolean;
}

export const ReasoningBlock: React.FC<ReasoningBlockProps> = ({
  content,
  isStreaming,
}) => {
  const [expanded, setExpanded] = React.useState(false);
  const { t } = useTranslation();

  return (
    <div className={css.root}>
      <button
        className={css.toggle}
        onClick={() => setExpanded((v) => !v)}
        type="button"
      >
        <span className={css.icon}>💭</span>
        <span>
          {expanded ? t("chat.reasoning.collapse") : t("chat.reasoning.expand")}
        </span>
        {isStreaming && (
          <span className={css.streaming}>{t("chat.reasoning.thinking")}</span>
        )}
      </button>
      {expanded && <pre className={css.content}>{content}</pre>}
    </div>
  );
};

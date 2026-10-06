import React from "react";
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

  return (
    <div className={css.root}>
      <button
        className={css.toggle}
        onClick={() => setExpanded((v) => !v)}
        type="button"
      >
        <span className={css.icon}>💭</span>
        <span>{expanded ? "收起思考" : "展开思考"}</span>
        {isStreaming && <span className={css.streaming}>思考中…</span>}
      </button>
      {expanded && <pre className={css.content}>{content}</pre>}
    </div>
  );
};

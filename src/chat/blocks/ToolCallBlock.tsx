import React from "react";
import { ToolOutlined } from "@ant-design/icons";
import type { ToolCall } from "../../stores/chatStore";
import css from "./ToolCallBlock.module.css";

interface ToolCallBlockProps {
  toolCall: ToolCall;
}

function tryFormatJson(text: string): string | null {
  try {
    return JSON.stringify(JSON.parse(text), null, 2);
  } catch {
    return null;
  }
}

export const ToolCallBlock: React.FC<ToolCallBlockProps> = ({ toolCall }) => {
  const [expanded, setExpanded] = React.useState(false);
  const argsPreview =
    toolCall.arguments.length > 120
      ? toolCall.arguments.slice(0, 120) + "…"
      : toolCall.arguments;

  return (
    <div className={css.root}>
      <button
        className={css.toggle}
        onClick={() => setExpanded((v) => !v)}
        type="button"
      >
        <ToolOutlined style={{ fontSize: 12, marginRight: 4 }} />
        <span className={css.name}>{toolCall.name || toolCall.id}</span>
      </button>
      {expanded && (
        <pre className={css.args}>
          {tryFormatJson(toolCall.arguments) || argsPreview}
        </pre>
      )}
    </div>
  );
};

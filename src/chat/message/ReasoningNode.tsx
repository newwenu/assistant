import React from "react";
import { useTranslation } from "react-i18next";
import { ReasoningBlock } from "../blocks/ReasoningBlock";
import css from "./ReasoningNode.module.css";

export type ReasoningNodeDensity = "single" | "block" | "expanded";

interface ReasoningNodeProps {
  text: string;
  density: ReasoningNodeDensity;
  isStreaming?: boolean;
}

/** 单行档只取首个非空行，不解析正文其余部分（门禁 8 的同口径）。 */
function firstLine(text: string): string {
  for (const line of text.split("\n")) {
    const trimmed = line.trim();
    if (trimmed.length > 0) return trimmed;
  }
  return "";
}

/**
 * 思考节点（I46 §6.1）：思考与工具步同权，属于同一个 run、共用组头的展开开关。
 * - `single`：run 收起态的单行；
 * - `expanded`：run 展开态，全文直接铺开；
 * - `block`：run 外的思考（流式中 / 无过程段），沿用 `ReasoningBlock` 的自身折叠。
 */
export const ReasoningNode: React.FC<ReasoningNodeProps> = ({
  text,
  density,
  isStreaming = false,
}) => {
  const { t } = useTranslation();

  if (density === "block") {
    return (
      <div className={css.root}>
        <ReasoningBlock content={text} isStreaming={isStreaming} />
      </div>
    );
  }

  const label = (
    <>
      <span className={css.icon}>💭</span>
      <span className={css.name}>{t("chat.message.group.thought")}</span>
      {isStreaming && (
        <span className={css.streaming}>{t("chat.reasoning.thinking")}</span>
      )}
    </>
  );

  if (density === "expanded") {
    return (
      <div className={css.root}>
        <div className={css.row}>{label}</div>
        <pre className={css.content}>{text}</pre>
      </div>
    );
  }

  return (
    <div className={css.root}>
      <div className={css.row}>
        {label}
        <span className={css.text}>{firstLine(text)}</span>
      </div>
    </div>
  );
};

import { useTranslation } from "react-i18next";
import type { TurnSummaryView } from "@/stores/agent/observation/projections/summary";
import {
  isAgentNodeId,
  isNodeState,
} from "@/stores/agent/observation/template";
import type {
  AgentNodeDetail,
  NodeFact,
} from "@/stores/agent/observation/types";
import styles from "./observation.module.css";

interface InspectorPanelProps {
  summary: TurnSummaryView;
  timeline: NodeFact[];
}

/**
 * 汇总 + 时间线（I55 §4.5 / §6.2）。
 *
 * 每条事实标注 `transition` 与 `source`；`derived` 值单独标「推导值」，
 * D 级字段只展示、不参与判定（I55 §6.6 第 5 条）。
 */
export function InspectorPanel({ summary, timeline }: InspectorPanelProps) {
  const { t } = useTranslation();
  const budget = summary.budget;

  return (
    <section className={styles.inspector}>
      <div className={styles.summaryPane}>
        <h4 className={styles.paneTitle}>
          {t("observation.inspector.summary")}
        </h4>
        <div className={styles.summaryList}>
          <SummaryRow
            label={t("observation.inspector.iteration")}
            value={String(summary.iteration)}
          />
          <SummaryRow
            label={t("observation.inspector.measured")}
            value={t("observation.inspector.milliseconds", {
              value: summary.measuredMs,
            })}
          />
          <SummaryRow
            label={t("observation.inspector.assumed_cells")}
            value={String(summary.assumedCells)}
          />
          {budget === null ? (
            <SummaryRow
              label={t("observation.inspector.budget")}
              value={t("observation.inspector.budget_none")}
            />
          ) : (
            <>
              <SummaryRow
                label={t("observation.inspector.budget_iterations")}
                value={optionText(budget.remainingIterations)}
              />
              <SummaryRow
                label={t("observation.inspector.budget_tools")}
                value={optionText(budget.remainingToolCalls)}
              />
              <SummaryRow
                label={t("observation.inspector.budget_wall_clock")}
                value={optionText(budget.remainingWallClockMs)}
              />
            </>
          )}
        </div>
      </div>
      <div className={styles.timelinePane}>
        <h4 className={styles.paneTitle}>
          {t("observation.inspector.timeline")}
        </h4>
        {timeline.length === 0 ? (
          <span className={styles.canvasHint}>
            {t("observation.inspector.no_facts")}
          </span>
        ) : (
          <div className={styles.timelineList}>
            {timeline.map((fact) => (
              <FactRow
                key={`${fact.seq}:${fact.node}:${fact.iteration}`}
                fact={fact}
              />
            ))}
          </div>
        )}
      </div>
    </section>
  );
}

function SummaryRow({ label, value }: { label: string; value: string }) {
  return (
    <div className={styles.summaryRow}>
      <span className={styles.summaryLabel}>{label}</span>
      <span className={styles.summaryValue}>{value}</span>
    </div>
  );
}

function FactRow({ fact }: { fact: NodeFact }) {
  const { t } = useTranslation();
  const nodeText = isAgentNodeId(fact.node)
    ? t(`observation.node.${fact.node}`)
    : fact.node;
  const stateText = isNodeState(fact.state)
    ? t(`observation.state.${fact.state}`)
    : fact.state;
  const entries = fact.detail === null ? [] : detailEntries(fact.detail);

  if (fact.synthetic) {
    return (
      <div className={styles.fact} data-transition="false">
        <span className={styles.factMeta}>{`… ${fact.omitted ?? 0}`}</span>
      </div>
    );
  }

  return (
    <div className={styles.fact} data-transition={fact.transition}>
      <div className={styles.factHead}>
        <span className={styles.factSeq}>{`#${fact.seq}`}</span>
        <span className={styles.factNode}>{nodeText}</span>
        <span className={styles.factState}>{stateText}</span>
        <span className={styles.cellBadge}>
          {fact.transition
            ? t("observation.inspector.transition")
            : t("observation.inspector.process")}
        </span>
        {fact.source !== "observed" && (
          <span className={styles.cellBadge}>
            {fact.source === "derived"
              ? t("observation.inspector.derived_value")
              : t(`observation.source.${fact.source}`)}
          </span>
        )}
        <span className={styles.factMeta}>
          {t("observation.inspector.milliseconds", { value: fact.atMs })}
        </span>
      </div>
      {entries.length > 0 && (
        <div className={styles.detailList}>
          {entries.map(([key, value]) => (
            <div className={styles.detailRow} key={key}>
              <span className={styles.detailKey}>{key}</span>
              <span>{value}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function optionText(value: number | null): string {
  return value === null ? "—" : String(value);
}

/** D 级字段逐条展示（I55 §4.5）；仅展示，不参与任何判定。 */
function detailEntries(detail: AgentNodeDetail): Array<[string, string]> {
  switch (detail.kind) {
    case "brake":
      return [
        ["verdict", detail.verdict],
        ["hook", optionText2(detail.hook)],
        ["remainingIterations", optionText(detail.budget.remainingIterations)],
        ["remainingToolCalls", optionText(detail.budget.remainingToolCalls)],
        [
          "remainingWallClockMs",
          optionText(detail.budget.remainingWallClockMs),
        ],
      ];
    case "ladder":
      return [
        ["applied", detail.applied.join(", ")],
        ["droppedMessages", optionText(detail.droppedMessages)],
        ["trimmedChars", optionText(detail.trimmedChars)],
      ];
    case "model":
      return [
        ["attempt", String(detail.attempt)],
        ["retryReason", optionText2(detail.retryReason)],
        ["promptTokens", optionText(detail.promptTokens)],
        ["completionTokens", optionText(detail.completionTokens)],
        ["finishReason", optionText2(detail.finishReason)],
        ["toolCallCount", String(detail.toolCallCount)],
      ];
    case "pipeline":
      return [
        ["stage", optionText2(detail.stage)],
        ["index", optionText(detail.index)],
        ["total", optionText(detail.total)],
        ["toolName", optionText2(detail.toolName)],
        ["toolCallId", optionText2(detail.toolCallId)],
      ];
    case "budgetSplit":
      return [
        ["requested", String(detail.requested)],
        ["executed", String(detail.executed)],
        ["deferred", String(detail.deferred)],
      ];
    case "branch":
      return [
        ["hasToolCalls", String(detail.hasToolCalls)],
        ["wrapUp", String(detail.wrapUp)],
        ["toolCallCount", String(detail.toolCallCount)],
      ];
    case "persist":
      return [
        ["rows", String(detail.rows)],
        ["messageIds", detail.messageIds.join(", ")],
      ];
    case "afterTool":
      return detail.verdicts.map((verdict, index): [string, string] => [
        `verdict[${index}]`,
        verdict.reason === null
          ? `${verdict.middleware} · ${verdict.action}`
          : `${verdict.middleware} · ${verdict.action} · ${verdict.reason}`,
      ]);
    case "afterIteration":
      return [
        ["nudges", detail.nudges.join(", ")],
        ["nextIteration", String(detail.nextIteration)],
      ];
  }
}

function optionText2(value: string | null): string {
  return value === null ? "—" : value;
}

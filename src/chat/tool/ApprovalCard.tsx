import React, { useEffect, useMemo, useState } from "react";
import { Button, Popconfirm } from "antd";
import { useTranslation } from "react-i18next";
import type {
  ApprovalExtension,
  ApprovalReason,
  ToolApprovalDecision,
  ToolApprovalRequest,
} from "../core/chatTypes";
import { chatService } from "@/services/chat/chatService";
import { sortApprovalExtensions } from "./toolCallDisplay";
import css from "./ApprovalCard.module.css";

type DecisionKind = ToolApprovalDecision["decision"];

interface ApprovalCardProps {
  approval: ToolApprovalRequest;
  startedAt?: number | null;
}

function formatRemaining(ms: number): string {
  const total = Math.max(0, Math.ceil(ms / 1000));
  const minutes = Math.floor(total / 60);
  const seconds = total % 60;
  return `${minutes}:${String(seconds).padStart(2, "0")}`;
}

function errorText(err: unknown): string {
  if (err instanceof Error) return err.message;
  return String(err);
}

/**
 * 审批卡片（I38d §3.2.2）。
 *
 * 只渲染后端给的信息，不推断缺失值：`preview = null` 时不显示量级行，
 * `extensions` 为空时不渲染扩展区——「未知」与「零」必须可区分。
 * 超时只置灰、不自动 `Deny`：决策以后端超时为准，避免双决策竞态。
 */
export const ApprovalCard: React.FC<ApprovalCardProps> = ({
  approval,
  startedAt,
}) => {
  const { t } = useTranslation();
  const [submitting, setSubmitting] = useState<DecisionKind | null>(null);
  const [error, setError] = useState<string | null>(null);

  const [mountedAt] = useState(() => Date.now());
  const deadline = useMemo(
    () => (startedAt ?? mountedAt) + approval.timeoutMs,
    [startedAt, mountedAt, approval.timeoutMs],
  );
  const [remainingMs, setRemainingMs] = useState(() => deadline - Date.now());

  useEffect(() => {
    setRemainingMs(deadline - Date.now());
    const timer = window.setInterval(() => {
      setRemainingMs(deadline - Date.now());
    }, 1000);
    return () => window.clearInterval(timer);
  }, [deadline]);

  const expired = remainingMs <= 0;
  const locked = expired || submitting !== null;

  const extensions = useMemo(
    () => sortApprovalExtensions(approval.context.extensions),
    [approval.context.extensions],
  );

  const decide = async (kind: DecisionKind) => {
    if (locked) return;
    setSubmitting(kind);
    setError(null);
    try {
      await chatService.resolveAgentToolApproval({
        conversationId: approval.conversationId,
        callId: approval.callId,
        decision: kind,
      });
    } catch (err) {
      setError(errorText(err));
      setSubmitting(null);
    }
  };

  const reasonText = (reason: ApprovalReason): string => {
    switch (reason.kind) {
      case "defaultPolicy":
        return t("chat.tool.approval.reason.default_policy", {
          rule: reason.rule,
        });
      case "approvalLevel":
        return t("chat.tool.approval.reason.approval_level", {
          level: t(`chat.tool.approval.level_hint.${reason.level}`),
        });
      case "projectRule":
        return t("chat.tool.approval.reason.project_rule", {
          action: reason.action,
          resource: reason.resource,
        });
      case "tierCeiling":
        return t("chat.tool.approval.reason.tier_ceiling", {
          tier: t(`chat.tool.approval.tier.${reason.tier}`),
        });
      case "advisor":
        return t("chat.tool.approval.reason.advisor", {
          rationale: reason.rationale,
        });
    }
  };

  const severityClass = (extension: ApprovalExtension): string => {
    switch (extension.severity) {
      case "danger":
        return css.severityDanger;
      case "caution":
        return css.severityCaution;
      default:
        return css.severityInfo;
    }
  };

  const preview = approval.context.preview;
  const behavior = approval.context.behavior;
  const tier = approval.context.tier;
  const tierClass =
    tier === "external"
      ? css.tierExternal
      : tier === "os"
        ? css.tierOs
        : css.tierSandbox;
  const hasImpact =
    behavior.readOnly || behavior.destructive || behavior.openWorld;

  return (
    <div className={css.root}>
      <div className={css.title}>
        <span>{t("chat.tool.approval.title")}</span>
        <span className={`${css.tier} ${tierClass}`}>
          {t(`chat.tool.approval.tier.${tier}`)}
        </span>
        <span className={css.timer}>
          {expired
            ? t("chat.tool.approval.expired")
            : t("chat.tool.approval.remaining", {
                time: formatRemaining(remainingMs),
              })}
        </span>
      </div>

      {preview && (
        <div className={css.row}>
          <span className={css.label}>{t("chat.tool.approval.preview")}</span>
          <span className={css.value}>
            {preview.truncated ? `≥ ${preview.summary}` : preview.summary}
          </span>
        </div>
      )}

      {approval.intents.length > 0 && (
        <div className={css.row}>
          <span className={css.label}>{t("chat.tool.approval.resources")}</span>
          <span className={css.value}>
            {approval.intents.map((intent, index) => (
              <span className={css.intent} key={`${intent.action}-${index}`}>
                <span className={css.intentAction}>{intent.action}</span>
                {intent.resources.join(", ")}
              </span>
            ))}
          </span>
        </div>
      )}

      {hasImpact && (
        <div className={css.row}>
          <span className={css.label}>{t("chat.tool.approval.impact")}</span>
          <span className={css.value}>
            {behavior.readOnly && (
              <span className={css.readOnly}>
                {t("chat.tool.approval.read_only")}
              </span>
            )}
            {behavior.destructive && (
              <span className={css.destructive}>
                {t("chat.tool.approval.destructive")}
              </span>
            )}
            {behavior.openWorld && (
              <span className={css.openWorld}>
                {t("chat.tool.approval.open_world")}
              </span>
            )}
          </span>
        </div>
      )}

      {extensions.length > 0 && (
        <div className={css.row}>
          <span className={css.label}>{t("chat.tool.approval.detection")}</span>
          <span className={css.value}>
            {extensions.map((extension, index) => (
              <span
                className={`${css.extension} ${severityClass(extension)}`}
                key={`${extension.kind}-${index}`}
              >
                <span className={css.extensionLabel}>{extension.label}</span>
                {extension.value}
              </span>
            ))}
          </span>
        </div>
      )}

      {approval.context.reasons.length > 0 && (
        <div className={css.row}>
          <span className={css.label}>{t("chat.tool.approval.reasons")}</span>
          <span className={css.value}>
            {approval.context.reasons.map((reason, index) => (
              <span className={css.reason} key={`${reason.kind}-${index}`}>
                {reasonText(reason)}
              </span>
            ))}
          </span>
        </div>
      )}

      {preview?.estimated && (
        <div className={css.note}>{t("chat.tool.approval.estimated")}</div>
      )}

      {error && <div className={css.error}>{error}</div>}

      <div className={css.actions}>
        <Button
          type="primary"
          size="small"
          disabled={locked}
          loading={submitting === "approve"}
          onClick={() => void decide("approve")}
        >
          {t("chat.tool.approval.approve")}
        </Button>
        <Button
          size="small"
          danger
          disabled={locked}
          loading={submitting === "deny"}
          onClick={() => void decide("deny")}
        >
          {t("chat.tool.approval.deny")}
        </Button>
        <Button
          size="small"
          disabled={locked}
          loading={submitting === "allowSession"}
          onClick={() => void decide("allowSession")}
        >
          {t("chat.tool.approval.session_allow")}
        </Button>
        <Popconfirm
          title={t("chat.tool.approval.always_confirm")}
          onConfirm={() => void decide("allowAlways")}
          disabled={locked}
        >
          <Button
            size="small"
            disabled={locked}
            loading={submitting === "allowAlways"}
          >
            {t("chat.tool.approval.always")}
          </Button>
        </Popconfirm>
        <span className={css.alwaysHint}>
          {t("chat.tool.approval.always_hint")}
        </span>
      </div>
    </div>
  );
};

import React, { useEffect, useMemo, useState } from "react";
import { Button, Input, Popconfirm, Tooltip } from "antd";
import {
  DownOutlined,
  ExclamationCircleOutlined,
  RightOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { translateKey } from "@/i18n";
import type {
  ApprovalReason,
  ToolApprovalDecision,
  ToolApprovalRequest,
} from "../core/chatTypes";
import { chatService } from "@/services/chat/chatService";
import { toChatError } from "../core/chatErrors";
import {
  buildApprovalView,
  type ApprovalFoldSectionId,
} from "./approvalPresentation";
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
  // `resolve_agent_tool_call` 已返回结构化错误：直接 `String(err)` 会得到
  // `[object Object]`，必须经 `toChatError` 取后端已本地化的 message。
  return toChatError(err).message ?? "";
}

/**
 * 审批卡片（I72 §4）。
 *
 * 四区块：决策句 / 命令块 / 按钮 / 折叠区。行 1 由视图模型求值（§5.1），
 * 卡片只负责呈现，不产生事实也不产生语义。折叠区按 §4.5 固定顺序，
 * 每项有内容才出现，入口文案按实际区块拼接。
 *
 * 超时只置灰、不自动 `Deny`：决策以后端超时为准，避免双决策竞态。
 */
export const ApprovalCard: React.FC<ApprovalCardProps> = ({
  approval,
  startedAt,
}) => {
  const { t } = useTranslation();
  const [submitting, setSubmitting] = useState<DecisionKind | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [foldOpen, setFoldOpen] = useState(false);
  const [denyReasonOpen, setDenyReasonOpen] = useState(false);
  const [denyReason, setDenyReason] = useState("");

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

  // 卡片每秒随倒计时重渲染，视图求值本身是纯函数且成本极低，不做 memo：
  // 省去语言切换时的失效依赖，也避免 `translateKey` 与 `t` 的类型割裂。
  const view = buildApprovalView(approval, translateKey);

  const expired = remainingMs <= 0;
  const locked = expired || submitting !== null;

  const decide = async (kind: DecisionKind, reason?: string) => {
    if (locked) return;
    setSubmitting(kind);
    setError(null);
    const payload: ToolApprovalDecision = {
      conversationId: approval.conversationId,
      callId: approval.callId,
      decision: kind,
    };
    if (kind === "deny" && reason !== undefined) payload.reason = reason;
    try {
      await chatService.resolveAgentToolApproval(payload);
    } catch (err) {
      // 失败保留理由框展开与已输入内容（§8.5 规则 7），不静默丢弃用户输入。
      setError(errorText(err));
      setSubmitting(null);
    }
  };

  /** 非拒绝动作一律不携带理由；展开的理由框随之收起、内容清空（§8.5 规则 3）。 */
  const decideOther = (kind: DecisionKind) => {
    setDenyReasonOpen(false);
    setDenyReason("");
    void decide(kind);
  };

  /** 拒绝：一次点击即提交；空白理由按「未填写」处理，不传空串（§8.5 规则 2）。 */
  const decideDeny = () => {
    const trimmed = denyReason.trim();
    void decide("deny", trimmed === "" ? undefined : trimmed);
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
      case "userRule":
        return t("chat.tool.approval.reason.user_rule", {
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

  const severityClass = (severity: string): string => {
    switch (severity) {
      case "danger":
        return css.severityDanger;
      case "caution":
        return css.severityCaution;
      default:
        return css.severityInfo;
    }
  };

  const tier = approval.context.tier;
  const showTier = tier === "os" || tier === "external";
  const tierClass = tier === "external" ? css.tierExternal : css.tierOs;

  const foldSections = view.detail.sections;

  const foldLabelKey = (section: ApprovalFoldSectionId): string =>
    section === "denyConsequence"
      ? "chat.tool.approval.fold.deny_consequence_label"
      : `chat.tool.approval.fold.${section}`;

  const renderFoldSection = (section: ApprovalFoldSectionId) => {
    switch (section) {
      case "why": {
        const { primary, rest } = view.detail.reasons;
        if (primary === null && rest.length === 0) return null;
        return (
          <div className={css.foldSection} key={section}>
            <span className={css.foldLabel}>
              {t("chat.tool.approval.fold.why")}
            </span>
            <div className={css.foldValue}>
              {primary !== null && <div>{reasonText(primary)}</div>}
              {rest.map((reason, index) => (
                <div key={`${reason.kind}-${index}`}>{reasonText(reason)}</div>
              ))}
            </div>
          </div>
        );
      }
      case "scope":
        return (
          <div className={css.foldSection} key={section}>
            <span className={css.foldLabel}>
              {t("chat.tool.approval.fold.scope")}
            </span>
            <div className={css.foldValue}>
              {view.detail.intents.map((intent, index) => (
                <div key={`${intent.action}-${index}`}>
                  <span className={css.foldAction}>{intent.label}</span>
                  {intent.raw.join(", ")}
                </div>
              ))}
            </div>
          </div>
        );
      case "evidence": {
        const evidence = view.detail.evidence;
        if (evidence === null) return null;
        return (
          <div className={css.foldSection} key={section}>
            <span className={css.foldLabel}>
              {t("chat.tool.approval.fold.evidence")}
            </span>
            <pre className={css.evidence}>{evidence.text}</pre>
            {evidence.truncated && (
              <span className={css.foldNote}>
                {t("chat.tool.approval.evidence.truncated")}
              </span>
            )}
          </div>
        );
      }
      case "arguments":
        return (
          <div className={css.foldSection} key={section}>
            <span className={css.foldLabel}>
              {t("chat.tool.approval.fold.arguments")}
            </span>
            <pre className={css.evidence}>{view.detail.arguments}</pre>
          </div>
        );
      case "diagnostics": {
        const diagnostics = view.detail.diagnostics;
        return (
          <div className={css.foldSection} key={section}>
            <span className={css.foldLabel}>
              {t("chat.tool.approval.fold.diagnostics")}
            </span>
            <div className={css.foldValue}>
              {diagnostics.readOnlyAnomaly && (
                <div>{t("chat.tool.approval.read_only")}</div>
              )}
              {diagnostics.previewSummary !== null && (
                <pre className={css.evidence}>{diagnostics.previewSummary}</pre>
              )}
              {diagnostics.previewUnavailable && (
                <div>
                  {t("chat.tool.approval.diagnostic.preview_unavailable")}
                </div>
              )}
              {diagnostics.extensions.map((extension, index) => (
                <div
                  className={severityClass(extension.severity)}
                  key={`${extension.kind}.${extension.code}-${index}`}
                >
                  {extension.label}
                </div>
              ))}
            </div>
          </div>
        );
      }
      case "denyConsequence":
        return (
          <div className={css.foldSection} key={section}>
            <span className={css.foldLabel}>
              {t("chat.tool.approval.fold.deny_consequence_label")}
            </span>
            <span className={css.foldValue}>{view.detail.denyConsequence}</span>
          </div>
        );
    }
  };

  return (
    <div className={css.root}>
      <div className={css.decision}>
        <span className={css.decisionText}>
          {expired ? (
            t("chat.tool.approval.expired_decision")
          ) : (
            <>
              {view.segments.length > 0 && (
                <span className={css.decisionVerb}>{view.segments[0]}</span>
              )}
              {view.segments.length > 1 && (
                <span>{` · ${view.segments.slice(1).join(" · ")}`}</span>
              )}
              {view.boundary !== null && (
                <span className={css.boundary}>
                  <ExclamationCircleOutlined />
                  {` ${view.boundary.text}`}
                </span>
              )}
            </>
          )}
        </span>
        {showTier && (
          <span className={`${css.tier} ${tierClass}`}>
            {t(`chat.tool.approval.tier.${tier}`)}
          </span>
        )}
        {!expired && (
          <span className={css.timer}>
            {t("chat.tool.approval.remaining", {
              time: formatRemaining(remainingMs),
            })}
          </span>
        )}
      </div>

      {view.command !== null && (
        <pre className={css.commandBlock}>{view.command}</pre>
      )}

      {error && <div className={css.error}>{error}</div>}

      <div className={css.actions}>
        <Button
          type="primary"
          size="small"
          disabled={locked}
          loading={submitting === "approve"}
          onClick={() => decideOther("approve")}
        >
          {t("chat.tool.approval.approve")}
        </Button>
        <Button
          size="small"
          danger
          disabled={locked}
          loading={submitting === "deny"}
          onClick={decideDeny}
        >
          {t("chat.tool.approval.deny")}
        </Button>
        <Tooltip title={view.scope.session}>
          <Button
            size="small"
            disabled={locked}
            loading={submitting === "allowSession"}
            onClick={() => decideOther("allowSession")}
          >
            {t("chat.tool.approval.session_allow")}
          </Button>
        </Tooltip>
        <Popconfirm
          title={t("chat.tool.approval.always_confirm", {
            intents: view.scope.alwaysIntents,
          })}
          onConfirm={() => decideOther("allowAlways")}
          disabled={locked}
        >
          <Tooltip title={view.scope.always}>
            <Button
              size="small"
              disabled={locked}
              loading={submitting === "allowAlways"}
            >
              {t("chat.tool.approval.always")}
            </Button>
          </Tooltip>
        </Popconfirm>
        {!expired && (
          <button
            className={css.denyReasonEntry}
            disabled={submitting !== null}
            onClick={() => setDenyReasonOpen((open) => !open)}
            type="button"
          >
            {denyReasonOpen
              ? t("chat.tool.approval.deny_reason_close")
              : t("chat.tool.approval.deny_reason_open")}
          </button>
        )}
      </div>

      {denyReasonOpen && !expired && (
        <div className={css.denyReasonField}>
          <Input
            autoFocus
            disabled={submitting !== null}
            maxLength={200}
            onChange={(event) => setDenyReason(event.target.value)}
            onPressEnter={(event) => event.preventDefault()}
            placeholder={t("chat.tool.approval.deny_reason_placeholder")}
            size="small"
            value={denyReason}
          />
          <span className={css.denyReasonHint}>
            {t("chat.tool.approval.deny_reason_hint")}
          </span>
        </div>
      )}

      {foldSections.length > 0 && (
        <div className={css.fold}>
          <button
            className={css.foldEntry}
            onClick={() => setFoldOpen((open) => !open)}
            type="button"
          >
            {foldOpen ? <DownOutlined /> : <RightOutlined />}
            <span>
              {foldSections
                .map((section) => translateKey(foldLabelKey(section)))
                .join(" · ")}
            </span>
          </button>
          {foldOpen && (
            <div className={css.foldBody}>
              {foldSections.map(renderFoldSection)}
            </div>
          )}
        </div>
      )}
    </div>
  );
};

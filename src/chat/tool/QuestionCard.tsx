import React, { useEffect, useMemo, useState } from "react";
import { Button, Input } from "antd";
import { useTranslation } from "react-i18next";
import type { AgentQuestion, AgentQuestionRequest } from "../core/chatTypes";
import { chatService } from "@/services/chatService";
import css from "./QuestionCard.module.css";

interface QuestionCardProps {
  question: AgentQuestionRequest;
  startedAt?: number | null;
}

/** 单题作答的本地状态：已选标签、是否选了「其他」、其他内容。 */
interface QuestionDraft {
  selected: string[];
  otherSelected: boolean;
  otherText: string;
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

function emptyDrafts(count: number): QuestionDraft[] {
  return Array.from({ length: count }, () => ({
    selected: [],
    otherSelected: false,
    otherText: "",
  }));
}

/** 一题是否算「已作答」：选了选项，或选了「其他」且填了内容。 */
function isAnswered(draft: QuestionDraft): boolean {
  return draft.selected.length > 0 || draft.otherText.trim().length > 0;
}

/**
 * 提问卡片（I38c §9）。
 *
 * 三条硬约束都体现在渲染里：
 * - 「其他」由前端自动追加，后端不传、模型也不能加，避免两套口径；
 * - 全部作答才允许提交，半答不算答案；
 * - 超时只置灰不自动提交：收口以后端超时为准，避免与用户点击竞态。
 *
 * `timeoutMs = 0` 表示不限时（配置为 0），此时不显示倒计时。
 */
export const QuestionCard: React.FC<QuestionCardProps> = ({
  question,
  startedAt,
}) => {
  const { t } = useTranslation();
  const [drafts, setDrafts] = useState<QuestionDraft[]>(() =>
    emptyDrafts(question.questions.length),
  );
  const [submitting, setSubmitting] = useState(false);
  const [submitted, setSubmitted] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const timed = question.timeoutMs > 0;
  const deadline = (startedAt ?? Date.now()) + question.timeoutMs;
  const [remainingMs, setRemainingMs] = useState(() =>
    timed ? deadline - Date.now() : 0,
  );

  useEffect(() => {
    if (!timed) return;
    setRemainingMs(deadline - Date.now());
    const timer = window.setInterval(() => {
      setRemainingMs(deadline - Date.now());
    }, 1000);
    return () => window.clearInterval(timer);
  }, [deadline, timed]);

  const expired = timed && remainingMs <= 0;
  const locked = expired || submitting || submitted;
  const complete = useMemo(
    () => drafts.every((draft) => isAnswered(draft)),
    [drafts],
  );

  const patch = (index: number, next: Partial<QuestionDraft>) => {
    if (locked) return;
    setDrafts((prev) =>
      prev.map((draft, i) => (i === index ? { ...draft, ...next } : draft)),
    );
  };

  const toggleOption = (index: number, spec: AgentQuestion, label: string) => {
    if (locked) return;
    const draft = drafts[index];
    if (!spec.multiSelect) {
      patch(index, { selected: [label], otherSelected: false });
      return;
    }
    const selected = draft.selected.includes(label)
      ? draft.selected.filter((item) => item !== label)
      : [...draft.selected, label];
    patch(index, { selected });
  };

  const toggleOther = (index: number, spec: AgentQuestion) => {
    if (locked) return;
    const draft = drafts[index];
    if (spec.multiSelect) {
      patch(index, { otherSelected: !draft.otherSelected });
      return;
    }
    patch(index, { selected: [], otherSelected: true });
  };

  const submit = async () => {
    if (locked || !complete) return;
    setSubmitting(true);
    setError(null);
    try {
      await chatService.resolveAgentQuestion({
        conversationId: question.conversationId,
        callId: question.callId,
        answers: drafts.map((draft) => ({
          selected: draft.selected,
          other: draft.otherSelected ? draft.otherText.trim() : null,
        })),
      });
      setSubmitted(true);
    } catch (err) {
      setError(errorText(err));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className={css.root}>
      <div className={css.title}>
        <span>{t("chat.tool.question.title")}</span>
        {timed && (
          <span className={css.timer}>
            {expired
              ? t("chat.tool.question.expired")
              : t("chat.tool.question.remaining", {
                  time: formatRemaining(remainingMs),
                })}
          </span>
        )}
      </div>

      {question.questions.map((spec, index) => {
        const draft = drafts[index];
        const otherActive = draft.otherSelected;
        return (
          <div className={css.question} key={`${spec.header}-${index}`}>
            <div className={css.questionHead}>
              <span className={css.chip}>{spec.header}</span>
              {spec.multiSelect && (
                <span className={css.multiHint}>
                  {t("chat.tool.question.multi_hint")}
                </span>
              )}
            </div>
            <div className={css.questionText}>{spec.question}</div>
            <div className={css.options}>
              {spec.options.map((option) => {
                const active = draft.selected.includes(option.label);
                return (
                  <button
                    type="button"
                    key={option.label}
                    className={`${css.option} ${active ? css.optionActive : ""}`}
                    disabled={locked}
                    onClick={() => toggleOption(index, spec, option.label)}
                  >
                    <span className={css.optionLabel}>{option.label}</span>
                    <span className={css.optionDesc}>{option.description}</span>
                  </button>
                );
              })}
              <div
                className={`${css.option} ${otherActive ? css.optionActive : ""}`}
                role="button"
                tabIndex={locked ? -1 : 0}
                onClick={() => toggleOther(index, spec)}
                onKeyDown={(event) => {
                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault();
                    toggleOther(index, spec);
                  }
                }}
              >
                <span className={css.optionLabel}>
                  {t("chat.tool.question.other")}
                </span>
                {otherActive && (
                  <Input
                    className={css.otherInput}
                    size="small"
                    autoFocus
                    disabled={locked}
                    value={draft.otherText}
                    placeholder={t("chat.tool.question.other_placeholder")}
                    onClick={(event) => event.stopPropagation()}
                    onChange={(event) =>
                      patch(index, { otherText: event.target.value })
                    }
                  />
                )}
              </div>
            </div>
          </div>
        );
      })}

      {error && <div className={css.error}>{error}</div>}

      <div className={css.actions}>
        <Button
          type="primary"
          size="small"
          disabled={locked || !complete}
          loading={submitting}
          onClick={() => void submit()}
        >
          {submitting
            ? t("chat.tool.question.submitting")
            : t("chat.tool.question.submit")}
        </Button>
        {!complete && (
          <span className={css.hint}>{t("chat.tool.question.hint")}</span>
        )}
      </div>
    </div>
  );
};

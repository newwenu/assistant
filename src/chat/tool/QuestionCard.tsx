import React, { useEffect, useMemo, useState } from "react";
import { Button, Input } from "antd";
import { useTranslation } from "react-i18next";
import type { AgentQuestion, AgentQuestionRequest } from "../core/chatTypes";
import { chatService } from "@/services/chat/chatService";
import { toChatError } from "../core/chatErrors";
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
  // `resolve_agent_question` 已返回结构化错误：直接 `String(err)` 会得到
  // `[object Object]`，必须经 `toChatError` 取后端已本地化的 message。
  return toChatError(err).message ?? "";
}

function newDraft(): QuestionDraft {
  return { selected: [], otherSelected: false, otherText: "" };
}

function emptyDrafts(count: number): QuestionDraft[] {
  return Array.from({ length: count }, newDraft);
}

/** 一题是否算「已作答」：选了选项，或「其他」处于选中态且填了内容。 */
function isAnswered(draft: QuestionDraft): boolean {
  if (draft.selected.length > 0) return true;
  return draft.otherSelected && draft.otherText.trim().length > 0;
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
 *
 * 倒计时锚点：`timeoutMs` 在补拉快照里已被后端改写为**剩余**毫秒，
 * 因此缺 `startedAt` 时以挂载时刻为锚点即可；`deadline` 必须固定，
 * 不能在渲染期现算，否则每次渲染都会重建 interval、倒计时永远不走。
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

  const questionCount = question.questions.length;
  const timed = question.timeoutMs > 0;
  const [mountedAt] = useState(() => Date.now());
  const deadline = useMemo(
    () => (startedAt ?? mountedAt) + question.timeoutMs,
    [startedAt, mountedAt, question.timeoutMs],
  );
  const [remainingMs, setRemainingMs] = useState(() =>
    timed ? deadline - Date.now() : 0,
  );

  /** 题数与 `drafts` 对齐：补拉重放可能带来不同的题数，越界会直接崩卡片。 */
  useEffect(() => {
    setDrafts((prev) => {
      if (prev.length === questionCount) return prev;
      const next = prev.slice(0, questionCount);
      while (next.length < questionCount) next.push(newDraft());
      return next;
    });
  }, [questionCount]);

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

  /** 所有改动都走函数式更新：连续点击不会读到渲染期的旧 `drafts`。 */
  const updateDraft = (
    index: number,
    updater: (draft: QuestionDraft) => QuestionDraft,
  ) => {
    if (locked) return;
    setDrafts((prev) =>
      prev.map((draft, i) => (i === index ? updater(draft) : draft)),
    );
  };

  const patch = (index: number, next: Partial<QuestionDraft>) => {
    updateDraft(index, (draft) => ({ ...draft, ...next }));
  };

  const toggleOption = (index: number, spec: AgentQuestion, label: string) => {
    updateDraft(index, (draft) => {
      if (!spec.multiSelect) {
        return { ...draft, selected: [label], otherSelected: false };
      }
      const selected = draft.selected.includes(label)
        ? draft.selected.filter((item) => item !== label)
        : [...draft.selected, label];
      return { ...draft, selected };
    });
  };

  const toggleOther = (index: number, spec: AgentQuestion) => {
    updateDraft(index, (draft) => {
      if (spec.multiSelect) {
        return { ...draft, otherSelected: !draft.otherSelected };
      }
      // 单选下「其他」是排他项，再点一次取消，与多选的可取消语义保持一致。
      return draft.otherSelected
        ? { ...draft, otherSelected: false }
        : { ...draft, selected: [], otherSelected: true };
    });
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
          other:
            draft.otherSelected && draft.otherText.trim().length > 0
              ? draft.otherText.trim()
              : null,
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
        const draft = drafts[index] ?? newDraft();
        const otherActive = draft.otherSelected;
        const markClass = spec.multiSelect
          ? css.optionMarkMulti
          : css.optionMarkSingle;
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
              {spec.options.map((option, optionIndex) => {
                const active = draft.selected.includes(option.label);
                return (
                  <button
                    type="button"
                    key={optionIndex}
                    className={`${css.option} ${active ? css.optionActive : ""}`}
                    disabled={locked}
                    aria-pressed={active}
                    onClick={() => toggleOption(index, spec, option.label)}
                  >
                    <span
                      className={`${css.optionMark} ${markClass} ${active ? css.optionMarkActive : ""}`}
                      aria-hidden="true"
                    />
                    <span className={css.optionBody}>
                      <span className={css.optionLabel}>{option.label}</span>
                      <span className={css.optionDesc}>
                        {option.description}
                      </span>
                    </span>
                  </button>
                );
              })}
              <div
                className={`${css.option} ${otherActive ? css.optionActive : ""}`}
                role="button"
                tabIndex={locked ? -1 : 0}
                aria-pressed={otherActive}
                onClick={() => toggleOther(index, spec)}
                onKeyDown={(event) => {
                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault();
                    toggleOther(index, spec);
                  }
                }}
              >
                <span
                  className={`${css.optionMark} ${markClass} ${otherActive ? css.optionMarkActive : ""}`}
                  aria-hidden="true"
                />
                <span className={css.optionBody}>
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
                </span>
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

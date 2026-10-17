import React, { useEffect, useState } from "react";
import { ReloadOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { translateKey } from "../../i18n";
import type { RetryNotification } from "../core/chatTypes";
import css from "./RetryBanner.module.css";

interface RetryBannerProps {
  retryStatus: RetryNotification;
  onCancel: () => void;
}

export const RetryBanner: React.FC<RetryBannerProps> = ({
  retryStatus,
  onCancel,
}) => {
  const { t } = useTranslation();
  const [countdown, setCountdown] = useState(
    Math.ceil(retryStatus.delayMs / 1000),
  );

  useEffect(() => {
    const timer = setInterval(() => {
      setCountdown((c) => Math.max(0, c - 1));
    }, 1000);
    return () => clearInterval(timer);
  }, []);

  return (
    <div className={css.banner}>
      <ReloadOutlined spin style={{ fontSize: 12, marginRight: 6 }} />
      <span>
        {translateKey("chat.retry.retrying", {
          attempt: retryStatus.attempt + 1,
          max: retryStatus.maxAttempts,
        })}
      </span>
      {countdown > 0 && (
        <span className={css.countdown}>
          {translateKey("chat.retry.countdown", { seconds: countdown })}
        </span>
      )}
      <button className={css.cancelBtn} onClick={onCancel} type="button">
        {t("common.actions.cancel")}
      </button>
    </div>
  );
};

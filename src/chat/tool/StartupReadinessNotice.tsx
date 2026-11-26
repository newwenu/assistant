import React from "react";
import { Button } from "antd";
import { InfoCircleOutlined, WarningOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { useStartupStore } from "@/stores/system/startupStore";
import css from "./StartupReadinessNotice.module.css";

/** 可操作降档项：仅 `tool_runtime` 需要前端提示（I75 §4.4 C5）。 */
const TOOL_RUNTIME = "tool_runtime";

/**
 * 工具能力就绪提示（I75 §4.4 C5）。
 *
 * `Pending` 显示「正在准备」，`Failed` 显示可读原因 + 重试入口；
 * `Ready` 与未水合时都不渲染，不占用布局。
 */
export const StartupReadinessNotice: React.FC = () => {
  const { t } = useTranslation();
  const capability = useStartupStore((s) => s.states[TOOL_RUNTIME]);
  const hydrated = useStartupStore((s) => s.hydrated);
  const retry = useStartupStore((s) => s.retry);

  if (!hydrated || !capability || capability.state === "ready") return null;

  if (capability.state === "pending") {
    return (
      <div className={css.root} role="status">
        <InfoCircleOutlined className={css.icon} />
        <span className={css.text}>{t("chat.tool.startup.pending")}</span>
      </div>
    );
  }

  return (
    <div className={css.root} role="alert">
      <WarningOutlined className={css.warnIcon} />
      <span className={css.text}>
        {t("chat.tool.startup.failed", {
          reason: capability.message ?? t("chat.tool.startup.unknown_reason"),
        })}
      </span>
      <Button
        size="small"
        type="link"
        className={css.action}
        onClick={() => void retry(TOOL_RUNTIME)}
      >
        {t("chat.tool.startup.retry")}
      </Button>
    </div>
  );
};

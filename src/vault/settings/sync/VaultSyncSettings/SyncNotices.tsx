import { Alert, Button, Space, Typography } from "antd";
import { useTranslation } from "react-i18next";

import type {
  PendingRemovals,
  UnreadableEntries,
  VaultErrorCode,
} from "../../../core/vault.types";

import { syncErrorText, type SyncResultInfo } from "./syncResult";

interface SyncNoticesProps {
  pendingRemovals: PendingRemovals | null;
  error: string | null;
  errorCode: VaultErrorCode | null;
  keyMismatchError: boolean;
  unreadableView: UnreadableEntries | null;
  resultInfo: SyncResultInfo | null;
  onReviewPendingRemovals: () => void;
  onDismissError: () => void;
  onGoRecovery: () => void;
}

/**
 * 同步过程中的四类提示：待删除条目、错误、不可读条目、本次同步结果。
 * 均只读展示 + 单一动作按钮，不参与同步编排。
 */
export const SyncNotices: React.FC<SyncNoticesProps> = ({
  pendingRemovals,
  error,
  errorCode,
  keyMismatchError,
  unreadableView,
  resultInfo,
  onReviewPendingRemovals,
  onDismissError,
  onGoRecovery,
}) => {
  const { t } = useTranslation();

  return (
    <>
      {pendingRemovals !== null && (
        <Alert
          type="warning"
          showIcon
          title={t("vault.sync.pending_removals_banner", {
            count: pendingRemovals.entries.length,
          })}
          description={t("vault.sync.pending_removals_hint")}
          action={
            <Button size="small" onClick={onReviewPendingRemovals}>
              {t("vault.sync.pending_removals_open")}
            </Button>
          }
        />
      )}

      {error && !keyMismatchError && (
        <Alert
          type="error"
          title={syncErrorText(errorCode, error)}
          showIcon
          closable={{ onClose: onDismissError }}
        />
      )}

      {unreadableView && (
        <Alert
          type="error"
          showIcon
          title={
            unreadableView.ids.length > 0
              ? t("vault.sync.unreadable_title", {
                  count: unreadableView.ids.length,
                })
              : t("vault.sync.unreadable_title_all")
          }
          description={
            <Space orientation="vertical" size={4} style={{ width: "100%" }}>
              <Typography.Text>
                {unreadableView.reason === "keyMismatch"
                  ? t("vault.sync.unreadable_reason_key_mismatch")
                  : t("vault.sync.unreadable_reason_corrupt")}
              </Typography.Text>
              {unreadableView.ids.length > 0 && (
                <Typography.Text
                  type="secondary"
                  style={{ wordBreak: "break-all" }}
                >
                  {unreadableView.ids.join(", ")}
                </Typography.Text>
              )}
              <Space>
                <Button size="small" onClick={onGoRecovery}>
                  {t("vault.sync.unreadable_export")}
                </Button>
                <Typography.Text
                  type="secondary"
                  style={{ fontSize: 12, alignSelf: "center" }}
                >
                  {t("vault.danger_zone.unreadable_overwrite_hint")}
                </Typography.Text>
              </Space>
            </Space>
          }
        />
      )}

      {resultInfo && (
        <Alert type={resultInfo.type} title={resultInfo.message} showIcon />
      )}
    </>
  );
};

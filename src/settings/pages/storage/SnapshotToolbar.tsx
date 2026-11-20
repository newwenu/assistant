import React from "react";
import { Alert, Button } from "antd";
import { ReloadOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { formatTimestamp } from "./format";

interface SnapshotToolbarProps {
  sampledAt: number;
  refreshing: boolean;
  error: string | null;
  onRefresh: () => void;
}

export const SnapshotToolbar: React.FC<SnapshotToolbarProps> = ({
  sampledAt,
  refreshing,
  error,
  onRefresh,
}) => {
  const { t } = useTranslation();

  return (
    <div className="storage-toolbar-wrap">
      <div className="storage-toolbar">
        <span className="storage-toolbar-meta">
          <span>
            {t("settings.storage.toolbar.sampled_at", {
              time: formatTimestamp(sampledAt),
            })}
          </span>
          <span className="storage-toolbar-sep">·</span>
          <span>{t("settings.storage.toolbar.byte_basis")}</span>
        </span>
        <Button
          size="small"
          icon={<ReloadOutlined />}
          loading={refreshing}
          onClick={onRefresh}
        >
          {t("settings.storage.toolbar.refresh")}
        </Button>
      </div>
      {error !== null && (
        <Alert
          type="warning"
          showIcon
          title={t("settings.storage.toolbar.refresh_failed")}
          description={error}
        />
      )}
    </div>
  );
};

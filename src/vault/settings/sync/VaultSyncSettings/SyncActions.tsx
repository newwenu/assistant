import { Button, Divider, Dropdown, Space, Tooltip, Typography } from "antd";
import {
  CloudDownloadOutlined,
  CloudUploadOutlined,
  ExclamationCircleOutlined,
  MoreOutlined,
  SyncOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";

interface SyncActionsProps {
  syncing: boolean;
  pushBlocked: boolean;
  onSyncNow: () => void;
  onSyncPull: () => void;
  onSyncPush: () => void;
  onGoDangerZone?: () => void;
}

/**
 * 同步动作区：立即同步、下拉里的 pull / push，以及 Danger Zone 入口。
 * `pushBlocked` 对应待删除条目尚未确认的场景。
 */
export const SyncActions: React.FC<SyncActionsProps> = ({
  syncing,
  pushBlocked,
  onSyncNow,
  onSyncPull,
  onSyncPush,
  onGoDangerZone,
}) => {
  const { t } = useTranslation();

  return (
    <>
      <Divider />

      <Space>
        <Button
          type="primary"
          icon={<SyncOutlined />}
          onClick={onSyncNow}
          loading={syncing}
        >
          {t("vault.sync.now")}
        </Button>
        <Dropdown
          disabled={syncing}
          menu={{
            items: [
              {
                key: "pull",
                icon: <CloudDownloadOutlined />,
                label: t("vault.sync.pull"),
                onClick: onSyncPull,
              },
              {
                key: "push",
                icon: <CloudUploadOutlined />,
                label: (
                  <Tooltip
                    title={t("vault.sync.push_hint_commits_first")}
                    placement="left"
                  >
                    <span>{t("vault.sync.push")}</span>
                  </Tooltip>
                ),
                disabled: pushBlocked,
                onClick: onSyncPush,
              },
            ],
          }}
        >
          <Button icon={<MoreOutlined />}>{t("common.actions.more")}</Button>
        </Dropdown>
      </Space>

      <Divider />

      <Space orientation="vertical" style={{ width: "100%" }} size="small">
        <Typography.Text strong type="danger">
          <ExclamationCircleOutlined style={{ marginRight: 6 }} />
          {t("vault.danger_zone.title")}
        </Typography.Text>
        <Typography.Text type="secondary">
          {t("vault.danger_zone.sync_hint")}
        </Typography.Text>
        {onGoDangerZone && (
          <Button size="small" danger onClick={onGoDangerZone}>
            {t("vault.danger_zone.open")}
          </Button>
        )}
      </Space>
    </>
  );
};

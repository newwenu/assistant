import { useEffect } from "react";
import { Drawer, List, Button, Empty, Typography, Tooltip, App } from "antd";
import { UndoOutlined, DeleteOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { useVaultStore } from "./core/vaultStore";
import { formatRelativeTime } from "@/utils/formatRelativeTime";
import styles from "./vault.module.css";

interface TrashDrawerProps {
  open: boolean;
  onClose: () => void;
}

export default function TrashDrawer({ open, onClose }: TrashDrawerProps) {
  const { t } = useTranslation();
  const { modal, message } = App.useApp();
  const { deletedEntries, loadDeletedEntries, restoreEntry, purgeDeleted } =
    useVaultStore();

  useEffect(() => {
    if (open) void loadDeletedEntries();
  }, [open, loadDeletedEntries]);

  const handleRestore = (id: string) => {
    modal.confirm({
      title: t("vault.entry.restore_confirm"),
      onOk: async () => {
        await restoreEntry(id);
        message.success(t("vault.entry.restored"));
      },
    });
  };

  const handlePurge = () => {
    modal.confirm({
      title: t("vault.entry.purge_confirm"),
      okType: "danger",
      onOk: async () => {
        const purged = await purgeDeleted();
        message.success(t("vault.entry.purged_count", { count: purged }));
      },
    });
  };

  return (
    <Drawer
      title={t("vault.entry.trash")}
      open={open}
      onClose={onClose}
      size={420}
      extra={
        <Button
          danger
          size="small"
          icon={<DeleteOutlined />}
          disabled={deletedEntries.length === 0}
          onClick={handlePurge}
        >
          {t("vault.entry.purge")}
        </Button>
      }
    >
      <Typography.Paragraph type="secondary" style={{ marginBottom: 16 }}>
        {t("vault.entry.trash_hint")}
      </Typography.Paragraph>

      {deletedEntries.length === 0 ? (
        <Empty description={t("vault.entry.trash_empty")} />
      ) : (
        <List
          dataSource={deletedEntries}
          className={styles.entryList}
          renderItem={(entry) => (
            <List.Item
              className={styles.entryItem}
              actions={[
                <Tooltip key="restore" title={t("vault.entry.restore")}>
                  <Button
                    type="text"
                    size="small"
                    icon={<UndoOutlined />}
                    onClick={() => handleRestore(entry.id)}
                  />
                </Tooltip>,
              ]}
            >
              <List.Item.Meta
                title={entry.title}
                description={
                  <span className={styles.entryDescription}>
                    {entry.username && (
                      <span className={styles.entryUsername}>
                        {entry.username}
                      </span>
                    )}
                    <span className={styles.entryTime}>
                      {formatRelativeTime(entry.updatedAt)}
                    </span>
                  </span>
                }
              />
            </List.Item>
          )}
        />
      )}
    </Drawer>
  );
}

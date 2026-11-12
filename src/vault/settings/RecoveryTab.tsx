import { useEffect, useState } from "react";
import { Alert, Button, Divider, Space, Typography } from "antd";
import { KeyOutlined, ReloadOutlined, SyncOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { useVaultShallow } from "../core/vaultStore";
import RecoveryKeyDialog from "./RecoveryKeyDialog";
import RotateDekDialog from "./RotateDekDialog";
import PaperRecoveryFlow from "../unlock/PaperRecoveryFlow";

export default function RecoveryTab() {
  const { t } = useTranslation();
  const { recoveryBackup, loadRecoveryBackup } = useVaultShallow((s) => ({
    recoveryBackup: s.recoveryBackup,
    loadRecoveryBackup: s.loadRecoveryBackup,
  }));
  const [exportOpen, setExportOpen] = useState(false);
  const [recoverOpen, setRecoverOpen] = useState(false);
  const [rotateOpen, setRotateOpen] = useState(false);

  useEffect(() => {
    void loadRecoveryBackup();
  }, [loadRecoveryBackup]);

  const exportedAt = recoveryBackup
    ? new Date(recoveryBackup.confirmedAt).toLocaleString()
    : null;

  return (
    <>
      <div>
        <Typography.Paragraph type="secondary">
          {t("vault.recovery.export.desc")}
        </Typography.Paragraph>

        {recoveryBackup ? (
          <Alert
            type="success"
            showIcon
            style={{ marginBottom: 16 }}
            title={t("vault.recovery.backupPresent", {
              version: recoveryBackup.dekVersion,
              time: exportedAt,
            })}
          />
        ) : (
          <Alert
            type="warning"
            showIcon
            style={{ marginBottom: 16 }}
            title={t("vault.recovery.backupMissing")}
          />
        )}

        <Space orientation="vertical" style={{ width: "100%" }}>
          <Button
            type="primary"
            icon={<KeyOutlined />}
            onClick={() => setExportOpen(true)}
          >
            {recoveryBackup
              ? t("vault.recovery.export.regenerate")
              : t("vault.recovery.export.title")}
          </Button>
          <Button icon={<SyncOutlined />} onClick={() => setRecoverOpen(true)}>
            {t("vault.recovery.paper.entry")}
          </Button>
        </Space>

        <Divider style={{ margin: "24px 0 16px" }} />

        <Space orientation="vertical" style={{ width: "100%" }} size="small">
          <Typography.Text strong type="danger">
            {t("vault.recovery.rotate.title")}
          </Typography.Text>
          <Typography.Paragraph
            type="secondary"
            style={{ margin: 0, fontSize: 12 }}
          >
            {t("vault.recovery.rotate.entry_desc")}
          </Typography.Paragraph>
          <Button
            danger
            icon={<ReloadOutlined />}
            onClick={() => setRotateOpen(true)}
          >
            {t("vault.recovery.rotate.entry")}
          </Button>
        </Space>
      </div>

      <RecoveryKeyDialog
        open={exportOpen}
        onClose={() => setExportOpen(false)}
      />
      <RotateDekDialog open={rotateOpen} onClose={() => setRotateOpen(false)} />
      <PaperRecoveryFlow
        open={recoverOpen}
        onClose={() => setRecoverOpen(false)}
      />
    </>
  );
}

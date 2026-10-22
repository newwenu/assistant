import { useState } from "react";
import { Modal, Radio, Space, Button, Typography } from "antd";
import { useTranslation } from "react-i18next";
import { useVaultStore } from "../core/vaultStore";
import type { ConflictResolution } from "../core/vault.service";

const { Text } = Typography;

interface VaultConflictResolveProps {
  open: boolean;
  conflictPath: string;
  onClose: () => void;
}

export default function VaultConflictResolve({
  open,
  conflictPath,
  onClose,
}: VaultConflictResolveProps) {
  const { t } = useTranslation();
  const { resolveConflict } = useVaultStore();
  const [resolution, setResolution] = useState<ConflictResolution>("useLocal");
  const [loading, setLoading] = useState(false);

  const handleResolve = async () => {
    setLoading(true);
    try {
      await resolveConflict(conflictPath, resolution);
      onClose();
    } finally {
      setLoading(false);
    }
  };

  return (
    <Modal
      open={open}
      title={t("tools.password_vault.conflict_title")}
      onCancel={onClose}
      footer={[
        <Button key="cancel" onClick={onClose}>
          {t("common.actions.cancel")}
        </Button>,
        <Button
          key="resolve"
          type="primary"
          loading={loading}
          onClick={() => void handleResolve()}
        >
          {t("tools.password_vault.conflict_resolve")}
        </Button>,
      ]}
    >
      <Space orientation="vertical" style={{ width: "100%" }}>
        <Text type="secondary">
          {t("tools.password_vault.conflict_file")}: {conflictPath}
        </Text>

        <Radio.Group
          value={resolution}
          onChange={(e) => setResolution(e.target.value as ConflictResolution)}
        >
          <Space orientation="vertical">
            <Radio value="useLocal">
              {t("tools.password_vault.conflict_use_local")}
            </Radio>
            <Radio value="useRemote">
              {t("tools.password_vault.conflict_use_remote")}
            </Radio>
            <Radio value="mergeBoth">
              {t("tools.password_vault.conflict_merge_both")}
            </Radio>
            <Radio value="skip">
              {t("tools.password_vault.conflict_skip")}
            </Radio>
          </Space>
        </Radio.Group>
      </Space>
    </Modal>
  );
}

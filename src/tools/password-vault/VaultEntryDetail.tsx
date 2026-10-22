import { useEffect } from "react";
import {
  Descriptions,
  Button,
  Typography,
  Space,
  Tag,
  message,
  Spin,
} from "antd";
import {
  CopyOutlined,
  EyeOutlined,
  StarFilled,
  StarOutlined,
  EditOutlined,
  DeleteOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { useVaultStore } from "./vaultStore";
import styles from "./password-vault.module.css";

interface VaultEntryDetailProps {
  entryId: string;
  onEdit: (id: string) => void;
  onClose: () => void;
}

export default function VaultEntryDetail({
  entryId,
  onEdit,
  onClose,
}: VaultEntryDetailProps) {
  const { t } = useTranslation();
  const {
    selectedEntry,
    revealedField,
    selectEntry,
    copyPassword,
    copyField,
    revealPassword,
    revealNotes,
    clearRevealedField,
    deleteEntry,
  } = useVaultStore();

  useEffect(() => {
    void selectEntry(entryId);
    return () => clearRevealedField();
  }, [entryId, selectEntry, clearRevealedField]);

  if (!selectedEntry) return <Spin />;

  const handleCopyUsername = () => {
    if (selectedEntry.username) {
      void navigator.clipboard.writeText(selectedEntry.username);
      message.success(t("common.actions.copied"));
    }
  };

  const handleCopyUrl = () => {
    if (selectedEntry.url) {
      void navigator.clipboard.writeText(selectedEntry.url);
      message.success(t("common.actions.copied"));
    }
  };

  const handleDelete = () => {
    void deleteEntry(entryId);
    onClose();
  };

  const handleCopyPassword = () => {
    void copyPassword(entryId);
  };

  const handleRevealPassword = () => {
    void revealPassword(entryId);
  };

  const handleRevealNotes = () => {
    void revealNotes(entryId);
  };

  const handleCopyField = (name: string) => {
    void copyField(entryId, name);
  };

  return (
    <div className={styles.detailContainer}>
      <div className={styles.detailHeader}>
        <Typography.Title level={4}>{selectedEntry.title}</Typography.Title>
        <Space>
          <Button
            type="text"
            icon={selectedEntry.isFavorite ? <StarFilled /> : <StarOutlined />}
          />
          <Button
            type="text"
            icon={<EditOutlined />}
            onClick={() => onEdit(entryId)}
          />
          <Button
            type="text"
            danger
            icon={<DeleteOutlined />}
            onClick={handleDelete}
          />
        </Space>
      </div>

      <Descriptions column={1} bordered size="small">
        <Descriptions.Item label={t("tools.password_vault.category")}>
          {t(`tools.password_vault.category_${selectedEntry.category}`)}
        </Descriptions.Item>

        {selectedEntry.username && (
          <Descriptions.Item label={t("tools.password_vault.username")}>
            <Space>
              <Typography.Text>{selectedEntry.username}</Typography.Text>
              <Button
                type="text"
                size="small"
                icon={<CopyOutlined />}
                onClick={handleCopyUsername}
              />
            </Space>
          </Descriptions.Item>
        )}

        {selectedEntry.hasPassword && (
          <Descriptions.Item label={t("tools.password_vault.password")}>
            <Space>
              {revealedField?.kind === "password" ? (
                <Typography.Text code>{revealedField.value}</Typography.Text>
              ) : (
                <Typography.Text type="secondary">••••••••</Typography.Text>
              )}
              <Button
                type="text"
                size="small"
                icon={<CopyOutlined />}
                onClick={handleCopyPassword}
              />
              <Button
                type="text"
                size="small"
                icon={<EyeOutlined />}
                onClick={handleRevealPassword}
              >
                {t("tools.password_vault.reveal")}
              </Button>
            </Space>
          </Descriptions.Item>
        )}

        {selectedEntry.url && (
          <Descriptions.Item label={t("tools.password_vault.url")}>
            <Space>
              <Typography.Text copyable>{selectedEntry.url}</Typography.Text>
              <Button
                type="text"
                size="small"
                icon={<CopyOutlined />}
                onClick={handleCopyUrl}
              />
            </Space>
          </Descriptions.Item>
        )}

        {selectedEntry.hasNotes && (
          <Descriptions.Item label={t("tools.password_vault.notes")}>
            <Space orientation="vertical">
              {revealedField?.kind === "notes" ? (
                <Typography.Text>{revealedField.value}</Typography.Text>
              ) : (
                <Typography.Text type="secondary">
                  {t("tools.password_vault.encrypted")}
                </Typography.Text>
              )}
              <Button
                type="text"
                size="small"
                icon={<EyeOutlined />}
                onClick={handleRevealNotes}
              >
                {t("tools.password_vault.reveal")}
              </Button>
            </Space>
          </Descriptions.Item>
        )}

        {selectedEntry.hasTotp && (
          <Descriptions.Item label="TOTP">
            <Typography.Text type="secondary">
              {t("tools.password_vault.totp_configured")}
            </Typography.Text>
          </Descriptions.Item>
        )}

        {selectedEntry.tags.length > 0 && (
          <Descriptions.Item label={t("tools.password_vault.tags")}>
            {selectedEntry.tags.map((tag) => (
              <Tag key={tag}>{tag}</Tag>
            ))}
          </Descriptions.Item>
        )}

        {selectedEntry.customFieldNames.length > 0 && (
          <Descriptions.Item label={t("tools.password_vault.custom_fields")}>
            <Space orientation="vertical">
              {selectedEntry.customFieldNames.map((name) => (
                <Space key={name}>
                  <Typography.Text strong>{name}</Typography.Text>
                  <Button
                    type="text"
                    size="small"
                    icon={<CopyOutlined />}
                    onClick={() => handleCopyField(name)}
                  />
                </Space>
              ))}
            </Space>
          </Descriptions.Item>
        )}

        <Descriptions.Item label={t("tools.password_vault.revision")}>
          {selectedEntry.revision}
        </Descriptions.Item>
      </Descriptions>
    </div>
  );
}

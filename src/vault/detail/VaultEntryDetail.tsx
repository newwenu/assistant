import { useEffect, useCallback } from "react";
import {
  Descriptions,
  Button,
  Typography,
  Space,
  Tag,
  App,
  Spin,
  Tooltip,
} from "antd";
import {
  CopyOutlined,
  StarFilled,
  StarOutlined,
  EditOutlined,
  DeleteOutlined,
  CloudUploadOutlined,
  ArrowLeftOutlined,
  UserOutlined,
  FileTextOutlined,
  CreditCardOutlined,
  IdcardOutlined,
  CodeOutlined,
  ToolOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { useVaultStore } from "../core/vaultStore";
import type { VaultCategory } from "../core/vault.types";
import TotpDisplay from "./TotpDisplay";
import RevealFieldButton from "./RevealFieldButton";
import styles from "../vault.module.css";

const CATEGORY_ICONS: Record<VaultCategory, React.ReactNode> = {
  login: <UserOutlined />,
  note: <FileTextOutlined />,
  card: <CreditCardOutlined />,
  identity: <IdcardOutlined />,
  sshKey: <CodeOutlined />,
  custom: <ToolOutlined />,
};

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
  const { modal, message } = App.useApp();
  const {
    selectedEntry,
    revealedField,
    totpCode,
    totpTimeRemaining,
    selectEntry,
    copyPassword,
    copyField,
    revealPassword,
    revealNotes,
    clearRevealedField,
    deleteEntry,
    updateEntry,
    sshAddToAgent,
    generateTotp,
    copyTotp,
    stopTotpTimer,
  } = useVaultStore();

  useEffect(() => {
    void selectEntry(entryId);
    return () => {
      clearRevealedField();
      stopTotpTimer();
    };
  }, [entryId, selectEntry, clearRevealedField, stopTotpTimer]);

  useEffect(() => {
    if (selectedEntry?.hasTotp) {
      void generateTotp(entryId);
    }
  }, [selectedEntry?.hasTotp, entryId, generateTotp]);

  const handleCopyTotp = useCallback(
    (id: string) => {
      void copyTotp(id);
      message.success(t("common.actions.copied"));
    },
    [copyTotp, t, message],
  );

  const handleToggleFavorite = useCallback(() => {
    if (!selectedEntry) return;
    const nextFav = !selectedEntry.isFavorite;
    void updateEntry(entryId, {
      category: selectedEntry.category,
      title: selectedEntry.title,
      isFavorite: nextFav,
      tags: selectedEntry.tags,
      customFields: selectedEntry.customFieldNames.map((name) => ({
        name,
        value: "",
        isProtected: false,
      })),
    }).then(() => {
      void selectEntry(entryId);
      message.success(
        t(nextFav ? "vault.entry.favorite" : "vault.entry.unfavorite"),
      );
    });
  }, [selectedEntry, entryId, updateEntry, selectEntry, t, message]);

  if (!selectedEntry) return <Spin />;

  const handleCopyUsername = () => {
    if (selectedEntry.username) {
      void navigator.clipboard.writeText(selectedEntry.username);
      message.success(t("common.actions.copied"));
    }
  };

  const handleDelete = () => {
    modal.confirm({
      title: t("vault.entry.confirm_delete"),
      okType: "danger",
      onOk: () => {
        void deleteEntry(entryId);
        onClose();
      },
    });
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

  const handleAddToSshAgent = () => {
    void sshAddToAgent(entryId).then((fp) => {
      if (fp) {
        message.success(t("vault.ssh.added_to_agent", { fingerprint: fp }));
      }
    });
  };

  return (
    <div className={styles.detailContainer}>
      <div className={styles.detailHeader}>
        <Space align="center" size={8}>
          <Button
            type="text"
            size="small"
            icon={<ArrowLeftOutlined />}
            onClick={onClose}
          />
          {CATEGORY_ICONS[selectedEntry.category]}
          <Typography.Title level={4} style={{ margin: 0 }}>
            {selectedEntry.title}
          </Typography.Title>
        </Space>
        <Space>
          <Tooltip
            title={
              selectedEntry.isFavorite
                ? t("vault.entry.unfavorite")
                : t("vault.entry.favorite")
            }
          >
            <Button
              type="text"
              icon={
                selectedEntry.isFavorite ? (
                  <StarFilled style={{ color: "#faad14" }} />
                ) : (
                  <StarOutlined />
                )
              }
              onClick={handleToggleFavorite}
            />
          </Tooltip>
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
        <Descriptions.Item label={t("vault.entry.category")}>
          {t(`vault.entry.category_${selectedEntry.category}`)}
        </Descriptions.Item>

        {selectedEntry.username && (
          <Descriptions.Item label={t("vault.entry.username")}>
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
          <Descriptions.Item label={t("vault.entry.password")}>
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
              <RevealFieldButton onClick={handleRevealPassword} />
            </Space>
          </Descriptions.Item>
        )}

        {selectedEntry.url && (
          <Descriptions.Item label={t("vault.entry.url")}>
            <Typography.Text copyable>{selectedEntry.url}</Typography.Text>
          </Descriptions.Item>
        )}

        {selectedEntry.hasNotes && (
          <Descriptions.Item label={t("vault.entry.notes")}>
            <Space orientation="vertical">
              {revealedField?.kind === "notes" ? (
                <Typography.Text>{revealedField.value}</Typography.Text>
              ) : (
                <Typography.Text type="secondary">
                  {t("vault.entry.encrypted")}
                </Typography.Text>
              )}
              <RevealFieldButton onClick={handleRevealNotes} />
            </Space>
          </Descriptions.Item>
        )}

        {selectedEntry.hasTotp && (
          <Descriptions.Item label="TOTP">
            <TotpDisplay
              entryId={entryId}
              code={totpCode}
              timeRemaining={totpTimeRemaining}
              period={30}
              onCopy={handleCopyTotp}
            />
          </Descriptions.Item>
        )}

        {selectedEntry.category === "sshKey" && (
          <Descriptions.Item label={t("vault.ssh.agent")}>
            <Button
              type="primary"
              size="small"
              icon={<CloudUploadOutlined />}
              onClick={handleAddToSshAgent}
            >
              {t("vault.ssh.add_to_agent")}
            </Button>
          </Descriptions.Item>
        )}

        {selectedEntry.tags.length > 0 && (
          <Descriptions.Item label={t("vault.entry.tags")}>
            {selectedEntry.tags.map((tag) => (
              <Tag key={tag}>{tag}</Tag>
            ))}
          </Descriptions.Item>
        )}

        {selectedEntry.customFieldNames.length > 0 && (
          <Descriptions.Item label={t("vault.entry.custom_fields")}>
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

        <Descriptions.Item label={t("vault.entry.revision")}>
          {selectedEntry.revision}
        </Descriptions.Item>
      </Descriptions>
    </div>
  );
}

import { useEffect, useCallback, useState } from "react";
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
  HistoryOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { useVaultShallow } from "../core/vaultStore";
import type { SshKeyMeta, VaultErrorCode } from "../core/vault.types";
import { vaultCategoryIcon } from "../core/vaultCategory";
import { VAULT_TONES } from "../core/vaultColors";
import TotpDisplay from "./TotpDisplay";
import RevealFieldButton from "./RevealFieldButton";
import VaultEntryHistory from "./VaultEntryHistory";
import styles from "../vault.module.css";

interface VaultEntryDetailProps {
  entryId: string;
  onEdit: (id: string) => void;
  onClose: () => void;
  showBack?: boolean;
}

export default function VaultEntryDetail({
  entryId,
  onEdit,
  onClose,
  showBack = true,
}: VaultEntryDetailProps) {
  const { t } = useTranslation();
  const { modal, message } = App.useApp();
  const {
    selectedEntry,
    revealedField,
    totpCode,
    totpTimeRemaining,
    totpPeriod,
    selectEntry,
    copyPassword,
    copyField,
    revealPassword,
    revealNotes,
    hideRevealedField,
    clearRevealedField,
    deleteEntry,
    setFavorite,
    sshAddToAgent,
    sshRemoveFromAgent,
    sshAgentKeys,
    sshListAgent,
    sshKeyMeta,
    generateTotp,
    copyTotp,
    stopTotpTimer,
  } = useVaultShallow((s) => ({
    selectedEntry: s.selectedEntry,
    revealedField: s.revealedField,
    totpCode: s.totpCode,
    totpTimeRemaining: s.totpTimeRemaining,
    totpPeriod: s.totpPeriod,
    selectEntry: s.selectEntry,
    copyPassword: s.copyPassword,
    copyField: s.copyField,
    revealPassword: s.revealPassword,
    revealNotes: s.revealNotes,
    hideRevealedField: s.hideRevealedField,
    clearRevealedField: s.clearRevealedField,
    deleteEntry: s.deleteEntry,
    setFavorite: s.setFavorite,
    sshAddToAgent: s.sshAddToAgent,
    sshRemoveFromAgent: s.sshRemoveFromAgent,
    sshAgentKeys: s.sshAgentKeys,
    sshListAgent: s.sshListAgent,
    sshKeyMeta: s.sshKeyMeta,
    generateTotp: s.generateTotp,
    copyTotp: s.copyTotp,
    stopTotpTimer: s.stopTotpTimer,
  }));

  const [sshMeta, setSshMeta] = useState<SshKeyMeta | null>(null);
  const [historyOpen, setHistoryOpen] = useState(false);
  const inAgent =
    !!sshMeta &&
    sshAgentKeys.some((k) => k.fingerprint === sshMeta.fingerprint);

  useEffect(() => {
    return () => {
      clearRevealedField();
      stopTotpTimer();
    };
  }, [clearRevealedField, stopTotpTimer]);

  const entryReady = selectedEntry?.id === entryId;

  useEffect(() => {
    if (!entryReady) void selectEntry(entryId);
  }, [entryReady, entryId, selectEntry]);

  useEffect(() => {
    if (selectedEntry?.category !== "sshKey") {
      setSshMeta(null);
      return;
    }
    let cancelled = false;
    void sshKeyMeta(entryId).then((meta) => {
      if (!cancelled) setSshMeta(meta);
    });
    void sshListAgent();
    return () => {
      cancelled = true;
    };
  }, [selectedEntry?.category, entryId, sshKeyMeta, sshListAgent]);

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
    void setFavorite(entryId, nextFav).then(() => {
      message.success(
        t(nextFav ? "vault.entry.favorite" : "vault.entry.unfavorite"),
      );
    });
  }, [selectedEntry, entryId, setFavorite, t, message]);

  const revealErrorText = useCallback(
    (code: VaultErrorCode | null) =>
      code === "rateLimited"
        ? t("vault.entry.reveal_rate_limited")
        : t("vault.entry.reveal_failed"),
    [t],
  );

  const handleRevealPassword = useCallback(async () => {
    const code = await revealPassword(entryId);
    if (code) message.error(revealErrorText(code));
  }, [entryId, revealPassword, message, revealErrorText]);

  const handleRevealNotes = useCallback(async () => {
    const code = await revealNotes(entryId);
    if (code) message.error(revealErrorText(code));
  }, [entryId, revealNotes, message, revealErrorText]);

  if (!selectedEntry || !entryReady) return <Spin />;

  const passwordRevealed =
    revealedField?.kind === "password" &&
    revealedField.visible &&
    revealedField.entryId === entryId;
  const notesRevealed =
    revealedField?.kind === "notes" &&
    revealedField.visible &&
    revealedField.entryId === entryId;

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
      onOk: () =>
        deleteEntry(entryId)
          .then(() => onClose())
          .catch(() => message.error(t("vault.entry.delete_failed"))),
    });
  };

  const handleCopyPassword = () => {
    void copyPassword(entryId);
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

  const handleRemoveFromSshAgent = () => {
    if (!sshMeta) return;
    void sshRemoveFromAgent(sshMeta.fingerprint).then(() => {
      message.success(t("vault.ssh.removed_from_agent"));
    });
  };

  return (
    <div className={styles.detailContainer}>
      <div className={styles.detailHeader}>
        <Space align="center" size={8}>
          {showBack && (
            <Button
              type="text"
              icon={<ArrowLeftOutlined />}
              onClick={onClose}
            />
          )}
          {vaultCategoryIcon(selectedEntry.category)}
          <Typography.Title level={4} style={{ margin: 0 }}>
            {selectedEntry.title}
          </Typography.Title>
        </Space>
        <Space size={4}>
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
                  <StarFilled style={{ color: VAULT_TONES.favorite }} />
                ) : (
                  <StarOutlined />
                )
              }
              onClick={handleToggleFavorite}
            />
          </Tooltip>
          <Tooltip title={t("common.actions.edit")}>
            <Button
              type="text"
              icon={<EditOutlined />}
              onClick={() => onEdit(entryId)}
            />
          </Tooltip>
          <Tooltip title={t("vault.history.open")}>
            <Button
              type="text"
              icon={<HistoryOutlined />}
              onClick={() => setHistoryOpen(true)}
            />
          </Tooltip>
          <span className={styles.headerActionDivider} />
          <Tooltip title={t("common.actions.delete")}>
            <Button
              type="text"
              danger
              icon={<DeleteOutlined />}
              onClick={handleDelete}
            />
          </Tooltip>
        </Space>
      </div>

      {selectedEntry.hasTotp && (
        <div className={styles.totpCard}>
          <Typography.Text type="secondary" className={styles.totpLabel}>
            {t("vault.totp.copy")}
          </Typography.Text>
          <TotpDisplay
            entryId={entryId}
            code={totpCode}
            timeRemaining={totpTimeRemaining}
            period={totpPeriod || selectedEntry.totpPeriod}
            onCopy={handleCopyTotp}
          />
        </div>
      )}

      <Descriptions
        column={1}
        size="small"
        bordered
        className={styles.detailFields}
      >
        {selectedEntry.username && (
          <Descriptions.Item label={t("vault.entry.username")}>
            <Space>
              <Typography.Text>{selectedEntry.username}</Typography.Text>
              <Button
                type="text"
                icon={<CopyOutlined />}
                onClick={handleCopyUsername}
              />
            </Space>
          </Descriptions.Item>
        )}

        {selectedEntry.hasPassword && (
          <Descriptions.Item label={t("vault.entry.password")}>
            <Space>
              {passwordRevealed ? (
                <Typography.Text code>{revealedField?.value}</Typography.Text>
              ) : (
                <Typography.Text type="secondary">••••••••</Typography.Text>
              )}
              <Button
                type="text"
                icon={<CopyOutlined />}
                onClick={handleCopyPassword}
              />
              <RevealFieldButton
                revealed={passwordRevealed}
                onReveal={handleRevealPassword}
                onHide={hideRevealedField}
              />
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
              {notesRevealed ? (
                <Typography.Text>{revealedField?.value}</Typography.Text>
              ) : (
                <Typography.Text type="secondary">
                  {t("vault.entry.encrypted")}
                </Typography.Text>
              )}
              <RevealFieldButton
                revealed={notesRevealed}
                onReveal={handleRevealNotes}
                onHide={hideRevealedField}
              />
            </Space>
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
                    icon={<CopyOutlined />}
                    onClick={() => handleCopyField(name)}
                  />
                </Space>
              ))}
            </Space>
          </Descriptions.Item>
        )}
      </Descriptions>

      {selectedEntry.category === "sshKey" && (
        <div className={styles.detailSection}>
          <Typography.Text strong className={styles.sectionTitle}>
            {t("vault.ssh.public_key")}
          </Typography.Text>
          {sshMeta && (
            <Descriptions
              column={2}
              size="small"
              className={styles.detailFields}
            >
              <Descriptions.Item label={t("vault.ssh.meta_key_type")}>
                {sshMeta.keyType}
              </Descriptions.Item>
              <Descriptions.Item label={t("vault.ssh.meta_bits")}>
                {sshMeta.bits}
              </Descriptions.Item>
              <Descriptions.Item
                label={t("vault.ssh.meta_fingerprint")}
                span={2}
              >
                <Typography.Text copyable code>
                  {sshMeta.fingerprint}
                </Typography.Text>
              </Descriptions.Item>
              <Descriptions.Item label={t("vault.ssh.meta_comment")} span={2}>
                {sshMeta.comment || "-"}
              </Descriptions.Item>
              <Descriptions.Item label={t("vault.ssh.public_key")} span={2}>
                <Typography.Text copyable code>
                  {sshMeta.publicKey}
                </Typography.Text>
              </Descriptions.Item>
            </Descriptions>
          )}
          <Space>
            <Button
              type="primary"
              icon={<CloudUploadOutlined />}
              onClick={handleAddToSshAgent}
            >
              {t("vault.ssh.add_to_agent")}
            </Button>
            {sshMeta && inAgent && (
              <Button
                danger
                icon={<DeleteOutlined />}
                onClick={handleRemoveFromSshAgent}
              >
                {t("vault.ssh.remove_from_agent")}
              </Button>
            )}
          </Space>
        </div>
      )}

      <div className={styles.detailMeta}>
        <span>
          {t("vault.entry.revision")} {selectedEntry.revision}
        </span>
      </div>

      <VaultEntryHistory
        open={historyOpen}
        entryId={entryId}
        onClose={() => setHistoryOpen(false)}
      />
    </div>
  );
}

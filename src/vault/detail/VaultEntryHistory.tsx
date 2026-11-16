import { useCallback, useEffect, useState } from "react";
import {
  Modal,
  Typography,
  Space,
  Button,
  Descriptions,
  Tag,
  Alert,
  Empty,
  Spin,
  App,
} from "antd";
import { useTranslation } from "react-i18next";
import { vaultService } from "../core/vault.service";
import { vaultCategoryLabel } from "../core/display/vaultCategory";
import { vaultErrorCode, vaultErrorMessage } from "../core/display/vaultError";
import { useVaultShallow } from "../core/vaultStore";
import type { EntryPreview, HistoryCommit } from "../core/vault.types";
import styles from "../vault.module.css";

const { Text } = Typography;

interface VaultEntryHistoryProps {
  open: boolean;
  entryId: string;
  onClose: () => void;
}

function formatTime(millis: number): string {
  return new Date(millis).toLocaleString();
}

function operationOf(message: string): string {
  const match = /^vault:\s*([a-z]+)/.exec(message.trim());
  return match ? match[1] : "other";
}

export default function VaultEntryHistory({
  open,
  entryId,
  onClose,
}: VaultEntryHistoryProps) {
  const { t } = useTranslation();
  const { message, modal } = App.useApp();
  const { loadEntries } = useVaultShallow((s) => ({
    loadEntries: s.loadEntries,
  }));

  const [commits, setCommits] = useState<HistoryCommit[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [preview, setPreview] = useState<EntryPreview | null>(null);
  const [loadingList, setLoadingList] = useState(false);
  const [loadingPreview, setLoadingPreview] = useState(false);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [restoring, setRestoring] = useState(false);

  useEffect(() => {
    if (!open) return;
    setCommits([]);
    setSelected(null);
    setPreview(null);
    setPreviewError(null);
    setLoadingList(true);
    vaultService
      .historyList(entryId, 50)
      .then((list) => setCommits(list))
      .catch((e: unknown) => {
        message.error(vaultErrorMessage(e));
      })
      .finally(() => setLoadingList(false));
  }, [open, entryId, message]);

  const loadPreview = useCallback(
    async (commit: string) => {
      setSelected(commit);
      setPreview(null);
      setPreviewError(null);
      setLoadingPreview(true);
      try {
        const result = await vaultService.historyPreview(commit, entryId);
        setPreview(result);
      } catch (e: unknown) {
        setPreviewError(
          vaultErrorCode(e) === "historyDecryptFailed"
            ? t("vault.history.old_key")
            : vaultErrorMessage(e),
        );
      } finally {
        setLoadingPreview(false);
      }
    },
    [entryId, t],
  );

  const operationLabel = (message: string): string => {
    switch (operationOf(message)) {
      case "create":
        return t("vault.history.op_create");
      case "update":
        return t("vault.history.op_update");
      case "delete":
        return t("vault.history.op_delete");
      case "restore":
        return t("vault.history.op_restore");
      default:
        return t("vault.history.op_other");
    }
  };

  const operationColor = (message: string): string => {
    switch (operationOf(message)) {
      case "delete":
        return "red";
      case "restore":
        return "green";
      default:
        return "blue";
    }
  };

  const handleRestore = () => {
    if (!selected) return;
    const target = commits.find((c) => c.hash === selected);
    modal.confirm({
      title: t("vault.history.restore"),
      content: t("vault.history.restore_confirm", {
        time: target ? formatTime(target.timestamp) : selected.slice(0, 8),
      }),
      onOk: async () => {
        setRestoring(true);
        try {
          await vaultService.historyRestore(selected, entryId);
          await loadEntries();
          message.success(t("vault.history.restored"));
          onClose();
        } catch (e: unknown) {
          message.error(
            vaultErrorCode(e) === "historyDecryptFailed"
              ? t("vault.history.old_key")
              : vaultErrorMessage(e),
          );
        } finally {
          setRestoring(false);
        }
      },
    });
  };

  return (
    <Modal
      open={open}
      title={t("vault.history.title")}
      onCancel={onClose}
      width={860}
      footer={[
        <Button key="close" onClick={onClose}>
          {t("common.actions.cancel")}
        </Button>,
        <Button
          key="restore"
          type="primary"
          disabled={!selected || !!previewError}
          loading={restoring}
          onClick={handleRestore}
        >
          {t("vault.history.restore")}
        </Button>,
      ]}
    >
      <div style={{ display: "flex", gap: 16, minHeight: 360 }}>
        <div style={{ width: 300, flexShrink: 0 }}>
          <Text type="secondary" style={{ display: "block", marginBottom: 8 }}>
            {t("vault.history.commits")}
          </Text>
          {loadingList ? (
            <Spin />
          ) : commits.length === 0 ? (
            <Empty description={t("vault.history.empty")} />
          ) : (
            <div className={styles.commitList}>
              {commits.map((item, index) => (
                <div
                  key={item.hash}
                  className={`${styles.commitItem} ${
                    selected === item.hash ? styles.commitItemActive : ""
                  }`}
                  onClick={() => void loadPreview(item.hash)}
                >
                  <Space size={6} wrap>
                    <Text style={{ fontSize: 13 }}>
                      {formatTime(item.timestamp)}
                    </Text>
                    <Tag
                      color={operationColor(item.message)}
                      style={{ marginInlineEnd: 0 }}
                    >
                      {operationLabel(item.message)}
                    </Tag>
                    {index === 0 && (
                      <Tag style={{ marginInlineEnd: 0 }}>
                        {t("vault.history.current")}
                      </Tag>
                    )}
                  </Space>
                  <Space orientation="vertical" size={0}>
                    <Text type="secondary" style={{ fontSize: 12 }}>
                      {item.message}
                    </Text>
                    <Text code style={{ fontSize: 11 }}>
                      {item.hash.slice(0, 8)}
                    </Text>
                  </Space>
                </div>
              ))}
            </div>
          )}
        </div>

        <div style={{ flex: 1, minWidth: 0 }}>
          <Text type="secondary" style={{ display: "block", marginBottom: 8 }}>
            {t("vault.history.preview")}
          </Text>
          {loadingPreview ? (
            <Spin />
          ) : previewError ? (
            <Alert type="error" showIcon title={previewError} />
          ) : !preview ? (
            <Empty description={t("vault.history.preview_hint")} />
          ) : (
            <Space orientation="vertical" size={8} style={{ width: "100%" }}>
              <Descriptions column={1} bordered size="small">
                <Descriptions.Item label={t("vault.entry.title")}>
                  {preview.title}
                </Descriptions.Item>
                <Descriptions.Item label={t("vault.entry.category")}>
                  {vaultCategoryLabel(t, preview.category)}
                </Descriptions.Item>
                {preview.username && (
                  <Descriptions.Item label={t("vault.entry.username")}>
                    {preview.username}
                  </Descriptions.Item>
                )}
                {preview.url && (
                  <Descriptions.Item label={t("vault.entry.url")}>
                    {preview.url}
                  </Descriptions.Item>
                )}
                {preview.password && (
                  <Descriptions.Item label={t("vault.entry.password")}>
                    <Text type="secondary">{preview.password}</Text>
                  </Descriptions.Item>
                )}
                {preview.totpSecret && (
                  <Descriptions.Item label={t("vault.entry.totp_secret")}>
                    <Text type="secondary">{preview.totpSecret}</Text>
                  </Descriptions.Item>
                )}
                {preview.notes && (
                  <Descriptions.Item label={t("vault.entry.notes")}>
                    <Text>{preview.notes}</Text>
                  </Descriptions.Item>
                )}
                {preview.customFields.map((field) => (
                  <Descriptions.Item key={field.name} label={field.name}>
                    {field.isSecret ? (
                      <Text type="secondary">{field.value}</Text>
                    ) : (
                      <Text>{field.value}</Text>
                    )}
                  </Descriptions.Item>
                ))}
                {preview.tags.length > 0 && (
                  <Descriptions.Item label={t("vault.entry.tags")}>
                    <Space wrap size={4}>
                      {preview.tags.map((tag) => (
                        <Tag key={tag}>{tag}</Tag>
                      ))}
                    </Space>
                  </Descriptions.Item>
                )}
                <Descriptions.Item label={t("vault.history.revision")}>
                  {preview.revision}
                </Descriptions.Item>
                <Descriptions.Item label={t("vault.history.time")}>
                  {formatTime(preview.updatedAt)}
                </Descriptions.Item>
              </Descriptions>
              <Text type="secondary" style={{ fontSize: 12 }}>
                {t("vault.history.masked_hint")}
              </Text>
            </Space>
          )}
        </div>
      </div>
    </Modal>
  );
}

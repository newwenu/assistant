import { useCallback, useEffect, useState } from "react";
import {
  Alert,
  App,
  Button,
  Modal,
  Radio,
  Space,
  Spin,
  Typography,
} from "antd";
import { useTranslation } from "react-i18next";

import { vaultService } from "../../../core/vault.service";
import { vaultErrorMessage } from "../../../core/display/vaultError";
import { useVaultShallow } from "../../../core/vaultStore";
import type {
  ConflictPreview,
  ConflictResolution,
} from "../../../core/vault.types";

import { ConflictCompare } from "./ConflictCompare";
import { useRevealSecrets } from "./useRevealSecrets";

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
  const { message, modal } = App.useApp();
  const { resolveConflict } = useVaultShallow((s) => ({
    resolveConflict: s.resolveConflict,
  }));
  const [resolution, setResolution] = useState<ConflictResolution>("useLocal");
  const [loading, setLoading] = useState(false);
  const [preview, setPreview] = useState<ConflictPreview | null>(null);
  const [previewLoading, setPreviewLoading] = useState(false);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [showUnchanged, setShowUnchanged] = useState(false);
  const {
    revealed,
    revealingKey,
    reveal,
    reset: resetReveal,
  } = useRevealSecrets(conflictPath, (text) => {
    message.error(text);
  });

  const loadPreview = useCallback(async () => {
    setPreviewLoading(true);
    setPreviewError(null);
    try {
      setPreview(await vaultService.conflictPreview(conflictPath));
    } catch (e) {
      setPreview(null);
      setPreviewError(vaultErrorMessage(e));
    } finally {
      setPreviewLoading(false);
    }
  }, [conflictPath]);

  useEffect(() => {
    if (!open) return;
    setResolution("useLocal");
    setPreview(null);
    setPreviewError(null);
    setShowUnchanged(false);
    resetReveal();
    void loadPreview();
  }, [open, loadPreview, resetReveal]);

  const performResolve = async () => {
    setLoading(true);
    try {
      await resolveConflict(
        conflictPath,
        resolution,
        resolution === "mergeBoth"
          ? t("vault.conflict.remote_copy_suffix")
          : undefined,
      );
      message.success(t("vault.conflict.resolved"));
      onClose();
    } finally {
      setLoading(false);
    }
  };

  const handleResolve = () => {
    if (resolution === "useLocal" || resolution === "useRemote") {
      modal.confirm({
        title: t("vault.conflict.confirm_title"),
        content:
          resolution === "useLocal"
            ? t("vault.conflict.confirm_use_local")
            : t("vault.conflict.confirm_use_remote"),
        okText: t("common.actions.confirm"),
        cancelText: t("common.actions.cancel"),
        onOk: () => performResolve(),
      });
      return;
    }
    void performResolve();
  };

  return (
    <Modal
      open={open}
      width={900}
      title={t("vault.conflict.title")}
      onCancel={onClose}
      footer={[
        <Button key="cancel" onClick={onClose}>
          {t("common.actions.cancel")}
        </Button>,
        <Button
          key="resolve"
          type="primary"
          loading={loading}
          onClick={handleResolve}
        >
          {t("vault.conflict.resolve")}
        </Button>,
      ]}
    >
      <Space orientation="vertical" size={12} style={{ width: "100%" }}>
        <Text type="secondary">
          {t("vault.conflict.file")}: {conflictPath}
        </Text>

        {previewLoading ? (
          <Space>
            <Spin size="small" />
            <Text type="secondary">{t("vault.conflict.preview_loading")}</Text>
          </Space>
        ) : previewError ? (
          <Alert
            type="warning"
            showIcon
            title={t("vault.conflict.preview_unavailable")}
            description={previewError}
          />
        ) : (
          preview && (
            <ConflictCompare
              preview={preview}
              showUnchanged={showUnchanged}
              onShowUnchangedChange={setShowUnchanged}
              t={t}
              reveal={{ revealed, revealingKey, reveal }}
            />
          )
        )}

        <Radio.Group
          value={resolution}
          onChange={(e) => setResolution(e.target.value as ConflictResolution)}
        >
          <Space orientation="vertical">
            <Radio value="useLocal">
              <Space orientation="vertical" size={0}>
                <Text>{t("vault.conflict.use_local")}</Text>
                <Text type="secondary" style={{ fontSize: 12 }}>
                  {t("vault.conflict.use_local_hint")}
                </Text>
              </Space>
            </Radio>
            <Radio value="useRemote">
              <Space orientation="vertical" size={0}>
                <Text>{t("vault.conflict.use_remote")}</Text>
                <Text type="secondary" style={{ fontSize: 12 }}>
                  {t("vault.conflict.use_remote_hint")}
                </Text>
              </Space>
            </Radio>
            <Radio value="mergeBoth">
              <Space orientation="vertical" size={0}>
                <Text>{t("vault.conflict.merge_both")}</Text>
                <Text type="secondary" style={{ fontSize: 12 }}>
                  {t("vault.conflict.merge_both_hint")}
                </Text>
              </Space>
            </Radio>
            <Radio value="skip">
              <Space orientation="vertical" size={0}>
                <Text>{t("vault.conflict.skip")}</Text>
                <Text type="secondary" style={{ fontSize: 12 }}>
                  {t("vault.conflict.skip_hint")}
                </Text>
              </Space>
            </Radio>
          </Space>
        </Radio.Group>
      </Space>
    </Modal>
  );
}

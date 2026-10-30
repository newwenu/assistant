import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import {
  Modal,
  Radio,
  Space,
  Button,
  Typography,
  App,
  Alert,
  Descriptions,
  Spin,
  Empty,
  Tag,
  Table,
  Checkbox,
} from "antd";
import { useTranslation } from "react-i18next";
import { vaultService } from "../core/vault.service";
import { vaultCategoryLabel } from "../core/vaultCategory";
import { vaultErrorMessage } from "../core/vaultError";
import { useVaultStore } from "../core/vaultStore";
import type {
  ConflictPreview,
  ConflictResolution,
  EntryPreview,
} from "../core/vault.types";

const { Text } = Typography;

const REVEAL_TIMEOUT_MS = 30_000;

type ConflictSide = "local" | "remote";

interface VaultConflictResolveProps {
  open: boolean;
  conflictPath: string;
  onClose: () => void;
}

interface CompareRow {
  key: string;
  label: string;
  local: ReactNode;
  remote: ReactNode;
  localPresent: boolean;
  remotePresent: boolean;
  changed: boolean;
}

function formatTime(millis: number): string {
  return new Date(millis).toLocaleString();
}

export default function VaultConflictResolve({
  open,
  conflictPath,
  onClose,
}: VaultConflictResolveProps) {
  const { t } = useTranslation();
  const { message, modal } = App.useApp();
  const { resolveConflict } = useVaultStore();
  const [resolution, setResolution] = useState<ConflictResolution>("useLocal");
  const [loading, setLoading] = useState(false);
  const [preview, setPreview] = useState<ConflictPreview | null>(null);
  const [previewLoading, setPreviewLoading] = useState(false);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [showUnchanged, setShowUnchanged] = useState(false);
  const [revealed, setRevealed] = useState<Record<string, string>>({});
  const [revealingKey, setRevealingKey] = useState<string | null>(null);
  const revealTimers = useRef<Record<string, number>>({});

  const clearRevealTimers = useCallback(() => {
    Object.values(revealTimers.current).forEach((timer) =>
      window.clearTimeout(timer),
    );
    revealTimers.current = {};
  }, []);

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
    setRevealed({});
    setRevealingKey(null);
    clearRevealTimers();
    void loadPreview();
  }, [open, loadPreview, clearRevealTimers]);

  useEffect(() => () => clearRevealTimers(), [clearRevealTimers]);

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

  const handleReveal = async (side: ConflictSide, field: string) => {
    const key = `${side}:${field}`;
    setRevealingKey(key);
    try {
      const value = await vaultService.revealConflictField(
        conflictPath,
        side,
        field,
      );
      setRevealed((prev) => ({ ...prev, [key]: value }));
      const timer = revealTimers.current[key];
      if (timer) window.clearTimeout(timer);
      revealTimers.current[key] = window.setTimeout(() => {
        delete revealTimers.current[key];
        setRevealed((prev) => {
          const next = { ...prev };
          delete next[key];
          return next;
        });
      }, REVEAL_TIMEOUT_MS);
    } catch (e) {
      message.error(vaultErrorMessage(e));
    } finally {
      setRevealingKey(null);
    }
  };

  const renderSecret = (side: ConflictSide, field: string, masked: string) => {
    const key = `${side}:${field}`;
    const value = revealed[key];
    return (
      <Space size={4} wrap>
        <Text
          type={value ? undefined : "secondary"}
          style={{ wordBreak: "break-all" }}
        >
          {value ?? masked}
        </Text>
        {!value && (
          <Button
            type="link"
            size="small"
            loading={revealingKey === key}
            onClick={() => void handleReveal(side, field)}
          >
            {t("vault.entry.reveal")}
          </Button>
        )}
      </Space>
    );
  };

  const renderTags = (tags: string[]) =>
    tags.length === 0 ? (
      <Text type="secondary">{t("vault.conflict.value_empty")}</Text>
    ) : (
      <Space wrap size={4}>
        {tags.map((tag) => (
          <Tag key={tag}>{tag}</Tag>
        ))}
      </Space>
    );

  const emptyNode = (
    <Text type="secondary">{t("vault.conflict.value_empty")}</Text>
  );

  const buildRows = (
    local: EntryPreview,
    remote: EntryPreview,
    diff: Set<string>,
  ): CompareRow[] => {
    const rows: CompareRow[] = [];

    const push = (
      key: string,
      label: string,
      localPresent: boolean,
      remotePresent: boolean,
      localNode: ReactNode,
      remoteNode: ReactNode,
    ) => {
      rows.push({
        key,
        label,
        local: localPresent ? localNode : emptyNode,
        remote: remotePresent ? remoteNode : emptyNode,
        localPresent,
        remotePresent,
        changed: diff.has(key),
      });
    };

    push(
      "title",
      t("vault.entry.title"),
      true,
      true,
      <Text>{local.title}</Text>,
      <Text>{remote.title}</Text>,
    );
    push(
      "category",
      t("vault.entry.category"),
      true,
      true,
      <Text>{vaultCategoryLabel(t, local.category)}</Text>,
      <Text>{vaultCategoryLabel(t, remote.category)}</Text>,
    );
    push(
      "username",
      t("vault.entry.username"),
      local.username !== null,
      remote.username !== null,
      <Text>{local.username}</Text>,
      <Text>{remote.username}</Text>,
    );
    push(
      "url",
      t("vault.entry.url"),
      local.url !== null,
      remote.url !== null,
      <Text>{local.url}</Text>,
      <Text>{remote.url}</Text>,
    );
    push(
      "password",
      t("vault.entry.password"),
      local.password !== null,
      remote.password !== null,
      renderSecret("local", "password", local.password ?? ""),
      renderSecret("remote", "password", remote.password ?? ""),
    );
    push(
      "totpSecret",
      t("vault.entry.totp_secret"),
      local.totpSecret !== null,
      remote.totpSecret !== null,
      renderSecret("local", "totpSecret", local.totpSecret ?? ""),
      renderSecret("remote", "totpSecret", remote.totpSecret ?? ""),
    );
    push(
      "notes",
      t("vault.entry.notes"),
      local.notes !== null,
      remote.notes !== null,
      <Text style={{ whiteSpace: "pre-wrap" }}>{local.notes}</Text>,
      <Text style={{ whiteSpace: "pre-wrap" }}>{remote.notes}</Text>,
    );

    const customNames = Array.from(
      new Set([
        ...local.customFields.map((field) => field.name),
        ...remote.customFields.map((field) => field.name),
      ]),
    );
    for (const name of customNames) {
      const localField = local.customFields.find(
        (field) => field.name === name,
      );
      const remoteField = remote.customFields.find(
        (field) => field.name === name,
      );
      push(
        `custom:${name}`,
        name,
        localField !== undefined,
        remoteField !== undefined,
        localField?.isSecret ? (
          renderSecret("local", name, localField.value)
        ) : (
          <Text>{localField?.value}</Text>
        ),
        remoteField?.isSecret ? (
          renderSecret("remote", name, remoteField.value)
        ) : (
          <Text>{remoteField?.value}</Text>
        ),
      );
    }

    push(
      "tags",
      t("vault.entry.tags"),
      true,
      true,
      renderTags(local.tags),
      renderTags(remote.tags),
    );
    push(
      "revision",
      t("vault.history.revision"),
      true,
      true,
      <Text>{local.revision}</Text>,
      <Text>{remote.revision}</Text>,
    );
    push(
      "updatedAt",
      t("vault.history.time"),
      true,
      true,
      <Text>{formatTime(local.updatedAt)}</Text>,
      <Text>{formatTime(remote.updatedAt)}</Text>,
    );
    push(
      "commitAuthor",
      t("vault.conflict.commit_author"),
      true,
      true,
      <Text>{local.commitAuthor ?? t("vault.conflict.value_empty")}</Text>,
      <Text>{remote.commitAuthor ?? t("vault.conflict.value_empty")}</Text>,
    );
    push(
      "commitTime",
      t("vault.conflict.commit_time"),
      true,
      true,
      <Text>
        {local.commitTime !== null
          ? formatTime(local.commitTime)
          : t("vault.conflict.value_empty")}
      </Text>,
      <Text>
        {remote.commitTime !== null
          ? formatTime(remote.commitTime)
          : t("vault.conflict.value_empty")}
      </Text>,
    );

    return rows;
  };

  const renderVersionCard = (
    label: string,
    side: ConflictSide,
    version: EntryPreview | null,
    versionError: string | null,
  ) => (
    <div style={{ flex: 1, minWidth: 0 }}>
      <Text type="secondary" style={{ display: "block", marginBottom: 8 }}>
        {label}
      </Text>
      {!version ? (
        <Empty
          description={versionError ?? t("vault.conflict.preview_empty")}
        />
      ) : (
        <Descriptions column={1} bordered size="small">
          <Descriptions.Item label={t("vault.entry.title")}>
            {version.title}
          </Descriptions.Item>
          <Descriptions.Item label={t("vault.entry.category")}>
            {vaultCategoryLabel(t, version.category)}
          </Descriptions.Item>
          {version.username && (
            <Descriptions.Item label={t("vault.entry.username")}>
              {version.username}
            </Descriptions.Item>
          )}
          {version.url && (
            <Descriptions.Item label={t("vault.entry.url")}>
              {version.url}
            </Descriptions.Item>
          )}
          {version.password && (
            <Descriptions.Item label={t("vault.entry.password")}>
              {renderSecret(side, "password", version.password)}
            </Descriptions.Item>
          )}
          {version.totpSecret && (
            <Descriptions.Item label={t("vault.entry.totp_secret")}>
              {renderSecret(side, "totpSecret", version.totpSecret)}
            </Descriptions.Item>
          )}
          {version.notes && (
            <Descriptions.Item label={t("vault.entry.notes")}>
              {version.notes}
            </Descriptions.Item>
          )}
          {version.customFields.map((field) => (
            <Descriptions.Item key={field.name} label={field.name}>
              {field.isSecret ? (
                renderSecret(side, field.name, field.value)
              ) : (
                <Text>{field.value}</Text>
              )}
            </Descriptions.Item>
          ))}
          {version.tags.length > 0 && (
            <Descriptions.Item label={t("vault.entry.tags")}>
              {renderTags(version.tags)}
            </Descriptions.Item>
          )}
          <Descriptions.Item label={t("vault.history.revision")}>
            {version.revision}
          </Descriptions.Item>
          <Descriptions.Item label={t("vault.history.time")}>
            {formatTime(version.updatedAt)}
          </Descriptions.Item>
          {version.commitAuthor && (
            <Descriptions.Item label={t("vault.conflict.commit_author")}>
              {version.commitAuthor}
            </Descriptions.Item>
          )}
          {version.commitTime !== null && (
            <Descriptions.Item label={t("vault.conflict.commit_time")}>
              {formatTime(version.commitTime)}
            </Descriptions.Item>
          )}
        </Descriptions>
      )}
    </div>
  );

  const renderCompare = (current: ConflictPreview) => {
    if (!current.local || !current.remote) {
      return (
        <div style={{ display: "flex", gap: 16 }}>
          {renderVersionCard(
            t("vault.conflict.preview_local"),
            "local",
            current.local,
            current.localError,
          )}
          {renderVersionCard(
            t("vault.conflict.preview_remote"),
            "remote",
            current.remote,
            current.remoteError,
          )}
        </div>
      );
    }

    const diff = new Set(current.diffFields);
    const rows = buildRows(current.local, current.remote, diff);
    const visibleRows = showUnchanged
      ? rows
      : rows.filter((row) => row.changed);
    const hiddenCount = rows.length - visibleRows.length;

    return (
      <Space orientation="vertical" size={8} style={{ width: "100%" }}>
        <Table<CompareRow>
          size="small"
          bordered
          pagination={false}
          rowKey="key"
          dataSource={visibleRows}
          onRow={(row) => ({
            style: row.changed
              ? { background: "rgba(255, 197, 61, 0.14)" }
              : undefined,
          })}
          columns={[
            {
              title: t("vault.conflict.field"),
              dataIndex: "label",
              width: 170,
              render: (_: unknown, row: CompareRow) => (
                <Space size={4} wrap>
                  <Text>{row.label}</Text>
                  {!row.localPresent && row.remotePresent && (
                    <Tag color="blue">{t("vault.conflict.only_remote")}</Tag>
                  )}
                  {row.localPresent && !row.remotePresent && (
                    <Tag color="orange">{t("vault.conflict.only_local")}</Tag>
                  )}
                  {row.localPresent && row.remotePresent && row.changed && (
                    <Tag color="red">{t("vault.conflict.changed")}</Tag>
                  )}
                </Space>
              ),
            },
            {
              title: t("vault.conflict.preview_local"),
              dataIndex: "local",
            },
            {
              title: t("vault.conflict.preview_remote"),
              dataIndex: "remote",
            },
          ]}
        />
        <Space size={8}>
          <Checkbox
            checked={showUnchanged}
            onChange={(e) => setShowUnchanged(e.target.checked)}
          >
            {t("vault.conflict.show_unchanged")}
          </Checkbox>
          {!showUnchanged && hiddenCount > 0 && (
            <Text type="secondary">
              {t("vault.conflict.unchanged_hidden", { count: hiddenCount })}
            </Text>
          )}
        </Space>
        <Text type="secondary" style={{ fontSize: 12 }}>
          {t("vault.conflict.preview_masked")}
        </Text>
      </Space>
    );
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
          preview && renderCompare(preview)
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

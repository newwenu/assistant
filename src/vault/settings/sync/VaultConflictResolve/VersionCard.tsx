import { Descriptions, Empty, Typography } from "antd";
import type { TFunction } from "i18next";

import { vaultCategoryLabel } from "../../../core/display/vaultCategory";
import type { EntryPreview } from "../../../core/vault.types";

import { renderSecret, renderTags } from "./secretCells";
import { formatTime, type ConflictSide, type RevealController } from "./types";

const { Text } = Typography;

interface VersionCardProps {
  label: string;
  side: ConflictSide;
  version: EntryPreview | null;
  versionError: string | null;
  t: TFunction;
  reveal: RevealController;
}

/**
 * 单侧版本卡片：只在某一侧无法解析时使用，展示该侧可读字段或错误占位。
 */
export const VersionCard: React.FC<VersionCardProps> = ({
  label,
  side,
  version,
  versionError,
  t,
  reveal,
}) => (
  <div style={{ flex: 1, minWidth: 0 }}>
    <Text type="secondary" style={{ display: "block", marginBottom: 8 }}>
      {label}
    </Text>
    {!version ? (
      <Empty description={versionError ?? t("vault.conflict.preview_empty")} />
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
            {renderSecret(reveal, side, "password", version.password, t)}
          </Descriptions.Item>
        )}
        {version.totpSecret && (
          <Descriptions.Item label={t("vault.entry.totp_secret")}>
            {renderSecret(reveal, side, "totpSecret", version.totpSecret, t)}
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
              renderSecret(reveal, side, field.name, field.value, t)
            ) : (
              <Text>{field.value}</Text>
            )}
          </Descriptions.Item>
        ))}
        {version.tags.length > 0 && (
          <Descriptions.Item label={t("vault.entry.tags")}>
            {renderTags(version.tags, t)}
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

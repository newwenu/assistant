import { Checkbox, Space, Table, Tag, Typography } from "antd";
import type { TFunction } from "i18next";

import type { ConflictPreview } from "../../../core/vault.types";

import { buildRows } from "./compareRows";
import type { CompareRow, RevealController } from "./types";
import { VersionCard } from "./VersionCard";

const { Text } = Typography;

interface ConflictCompareProps {
  preview: ConflictPreview;
  showUnchanged: boolean;
  onShowUnchangedChange: (checked: boolean) => void;
  t: TFunction;
  reveal: RevealController;
}

/**
 * 冲突对比区：两侧都能解析时用表格逐字段对比，否则退化为两张版本卡片。
 */
export const ConflictCompare: React.FC<ConflictCompareProps> = ({
  preview,
  showUnchanged,
  onShowUnchangedChange,
  t,
  reveal,
}) => {
  if (!preview.local || !preview.remote) {
    return (
      <div style={{ display: "flex", gap: 16 }}>
        <VersionCard
          label={t("vault.conflict.preview_local")}
          side="local"
          version={preview.local}
          versionError={preview.localError}
          t={t}
          reveal={reveal}
        />
        <VersionCard
          label={t("vault.conflict.preview_remote")}
          side="remote"
          version={preview.remote}
          versionError={preview.remoteError}
          t={t}
          reveal={reveal}
        />
      </div>
    );
  }

  const diff = new Set(preview.diffFields);
  const rows = buildRows(preview.local, preview.remote, diff, t, reveal);
  const visibleRows = showUnchanged ? rows : rows.filter((row) => row.changed);
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
          onChange={(e) => onShowUnchangedChange(e.target.checked)}
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

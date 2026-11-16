import type { ReactNode } from "react";
import { Button, Space, Tag, Typography } from "antd";
import type { TFunction } from "i18next";

import type { ConflictSide, RevealController } from "./types";

const { Text } = Typography;

export function renderSecret(
  controller: RevealController,
  side: ConflictSide,
  field: string,
  masked: string,
  t: TFunction,
): ReactNode {
  const key = `${side}:${field}`;
  const value = controller.revealed[key];
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
          loading={controller.revealingKey === key}
          onClick={() => controller.reveal(side, field)}
        >
          {t("vault.entry.reveal")}
        </Button>
      )}
    </Space>
  );
}

export function renderTags(tags: string[], t: TFunction): ReactNode {
  return tags.length === 0 ? (
    <Text type="secondary">{t("vault.conflict.value_empty")}</Text>
  ) : (
    <Space wrap size={4}>
      {tags.map((tag) => (
        <Tag key={tag}>{tag}</Tag>
      ))}
    </Space>
  );
}

export function emptyValue(t: TFunction): ReactNode {
  return <Text type="secondary">{t("vault.conflict.value_empty")}</Text>;
}

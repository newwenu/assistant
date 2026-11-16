import React from "react";
import { Typography } from "antd";
import { useTranslation } from "react-i18next";

import type { ModelConfig } from "../types";

interface ModelPreviewListProps {
  models: ModelConfig[];
  onSelect: (id: string) => void;
}

/** 获取到的模型清单预览：点击任意一项即回填默认模型。 */
export const ModelPreviewList: React.FC<ModelPreviewListProps> = ({
  models,
  onSelect,
}) => {
  const { t } = useTranslation();

  if (models.length === 0) return null;

  return (
    <div className="connect-model-list">
      {models.slice(0, 20).map((m) => (
        <button
          key={m.id}
          type="button"
          className="connect-model-item"
          onClick={() => onSelect(m.id)}
        >
          <span className="connect-model-name">{m.name || m.id}</span>
          <span className="connect-model-id">{m.id}</span>
          {m.attachment ? (
            <span className="connect-model-attachment">
              {t(
                `settings.provider.field.attachment_support.${m.attachment.support}`,
              )}
            </span>
          ) : null}
        </button>
      ))}
      {models.length > 20 && (
        <Typography.Text
          type="secondary"
          style={{ fontSize: 11, padding: "4px 0" }}
        >
          {t("settings.provider.field.models_more", {
            count: models.length - 20,
          })}
        </Typography.Text>
      )}
    </div>
  );
};

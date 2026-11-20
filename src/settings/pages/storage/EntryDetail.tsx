import React from "react";
import { Tag } from "antd";
import { useTranslation } from "react-i18next";
import type { EntryDetailView } from "./entryView";

interface EntryDetailProps {
  id: string;
  detail: EntryDetailView;
}

export const EntryDetail: React.FC<EntryDetailProps> = ({ id, detail }) => {
  const { t } = useTranslation();

  return (
    <div className="storage-entry-detail" id={id}>
      <div className="storage-entry-detail-grid">
        {detail.items.map((item, index) => (
          <div
            className="storage-entry-detail-item"
            key={`${index}-${item.value}`}
          >
            {item.label !== null && (
              <span className="storage-entry-detail-label">{item.label}</span>
            )}
            <span
              className={
                item.mono
                  ? "storage-entry-detail-value is-mono"
                  : "storage-entry-detail-value"
              }
            >
              {item.value}
            </span>
          </div>
        ))}
        {detail.availabilityReason !== null && (
          <div className="storage-entry-detail-item">
            <span className="storage-entry-detail-label">
              {t("settings.storage.entries.facts.availability_reason")}
            </span>
            <span className="storage-entry-detail-value">
              {detail.availabilityReason}
            </span>
          </div>
        )}
        {detail.note !== null && (
          <div className="storage-entry-detail-item">
            <span className="storage-entry-detail-label">
              {t("settings.storage.entries.facts.note")}
            </span>
            <span className="storage-entry-detail-value">{detail.note}</span>
          </div>
        )}
      </div>
      {detail.chips.length > 0 && (
        <div className="storage-entry-detail-chips">
          {detail.chips.map((chip) => (
            <Tag key={chip}>{chip}</Tag>
          ))}
        </div>
      )}
    </div>
  );
};

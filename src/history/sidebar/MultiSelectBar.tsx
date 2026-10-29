import React from "react";
import { useTranslation } from "react-i18next";
import { Button, Popconfirm } from "antd";
import { DeleteOutlined, CloseOutlined } from "@ant-design/icons";
import css from "./MultiSelectBar.module.css";

interface MultiSelectBarProps {
  selectedCount: number;
  totalCount: number;
  allIds: string[];
  selectedIds: Set<string>;
  onBatchDelete: (ids: string[]) => void;
  onBatchTogglePin: (ids: string[]) => void;
  onBatchExport: (ids: string[]) => void;
  onSelectAll: (ids: string[]) => void;
  onClearSelections: () => void;
  onExit: () => void;
}

export const MultiSelectBar: React.FC<MultiSelectBarProps> = ({
  selectedCount,
  totalCount,
  allIds,
  selectedIds,
  onBatchDelete,
  onBatchTogglePin,
  onBatchExport,
  onSelectAll,
  onClearSelections,
  onExit,
}) => {
  const { t } = useTranslation();
  const ids = Array.from(selectedIds);
  const noneSelected = selectedCount === 0;

  return (
    <div className={css.bar}>
      <div className={css.info}>
        <span className={css.selectedCount}>
          {t("history.selected_count", { count: selectedCount })}
        </span>
        <div className={css.selectAll}>
          {selectedCount === totalCount && totalCount > 0 ? (
            <Button size="small" type="link" onClick={onClearSelections}>
              {t("history.deselect_all")}
            </Button>
          ) : (
            <Button
              size="small"
              type="link"
              onClick={() => onSelectAll(allIds)}
            >
              {t("history.select_all")}
            </Button>
          )}
        </div>
      </div>
      <div className={css.actions}>
        <Button
          size="small"
          type="text"
          disabled={noneSelected}
          onClick={() => onBatchTogglePin(ids)}
        >
          {t("history.batch_pin")}
        </Button>
        <Button
          size="small"
          type="text"
          disabled={noneSelected}
          onClick={() => onBatchExport(ids)}
        >
          {t("history.batch_export")}
        </Button>
        <Popconfirm
          title={t("history.batch_delete_confirm", { count: selectedCount })}
          onConfirm={() => onBatchDelete(ids)}
          disabled={noneSelected}
        >
          <Button
            size="small"
            danger
            icon={<DeleteOutlined />}
            disabled={noneSelected}
          >
            {t("common.actions.delete")}
          </Button>
        </Popconfirm>
        <div className={css.close}>
          <Button
            size="small"
            type="text"
            icon={<CloseOutlined />}
            onClick={onExit}
          />
        </div>
      </div>
    </div>
  );
};
